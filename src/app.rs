// SPDX-License-Identifier: MPL-2.0

use std::time::Duration;

use crate::config::Config;
use crate::fl;
use cosmic::Theme;
use cosmic::cosmic_config::{self, CosmicConfigEntry};
use cosmic::iced::futures::SinkExt;
use cosmic::iced::{Alignment, Length, Limits, Subscription, window::Id};
use cosmic::iced_winit::commands::popup::{destroy_popup, get_popup};
use cosmic::prelude::*;
use cosmic::widget::{self, container};

const TOMATO_SVG: &[u8] = include_bytes!("../resources/tomato.svg");
const PAUSE_SVG: &[u8] = include_bytes!("../resources/pause.svg");

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Phase {
    #[default]
    Idle,
    Work,
    ShortBreak,
    LongBreak,
}

impl Phase {
    fn total_secs(self, config: &Config) -> u32 {
        match self {
            Self::Idle | Self::Work => config.work_mins * 60,
            Self::ShortBreak => config.short_break_mins * 60,
            Self::LongBreak => config.long_break_mins * 60,
        }
    }
}

#[derive(Default)]
pub struct AppModel {
    core: cosmic::Core,
    popup: Option<Id>,
    config: Config,
    config_handler: Option<cosmic_config::Config>,
    phase: Phase,
    remaining_secs: u32,
    paused: bool,
    completed_pomodoros: u32,
}

#[derive(Debug, Clone)]
pub enum Message {
    TogglePopup,
    ToggleTimer,
    PopupClosed(Id),
    UpdateConfig(Config),
    Tick,
    Start,
    Pause,
    Resume,
    Reset,
    Skip,
    SetWorkMins(u32),
    SetShortBreakMins(u32),
    SetLongBreakMins(u32),
    SetLongBreakInterval(u32),
}

impl AppModel {
    fn start_phase(&mut self, phase: Phase) {
        self.phase = phase;
        self.remaining_secs = match phase {
            Phase::Idle => 0,
            _ => phase.total_secs(&self.config),
        };
        self.paused = false;
    }

    fn start_timer(&mut self) {
        self.start_phase(Phase::Work);
        self.completed_pomodoros = 0;
    }

    fn reset_timer(&mut self) {
        self.start_phase(Phase::Idle);
        self.completed_pomodoros = 0;
    }

    #[allow(clippy::cast_precision_loss)]
    fn progress(&self) -> f32 {
        let total_secs = self.phase.total_secs(&self.config);
        if total_secs > 0 && self.phase != Phase::Idle {
            1.0 - (self.remaining_secs as f32 / total_secs as f32)
        } else {
            0.0
        }
    }

    fn completed_in_cycle(&self, goal: u32) -> u32 {
        match self.phase {
            Phase::Work => (self.completed_pomodoros % goal) + 1,
            Phase::LongBreak => goal,
            _ => self.completed_pomodoros % goal,
        }
    }

    fn update_config_value(&mut self, value: u32, update: impl FnOnce(&mut Config, u32)) {
        update(&mut self.config, value);
        self.save_config();
    }

    fn controls(&self) -> Element<'_, Message> {
        let controls = match (self.phase, self.paused) {
            (Phase::Idle, _) => {
                widget::row().push(widget::button::suggested(fl!("start")).on_press(Message::Start))
            }
            (_, true) => widget::row()
                .push(widget::button::suggested(fl!("resume")).on_press(Message::Resume))
                .push(widget::button::destructive(fl!("reset")).on_press(Message::Reset)),
            (_, false) => widget::row()
                .push(widget::button::standard(fl!("pause")).on_press(Message::Pause))
                .push(widget::button::standard(fl!("skip")).on_press(Message::Skip))
                .push(widget::button::destructive(fl!("reset")).on_press(Message::Reset)),
        };

        controls.spacing(8).into()
    }

    fn advance_phase(&mut self) {
        match self.phase {
            Phase::Work => {
                self.completed_pomodoros += 1;
                let next_phase = if self.config.long_break_interval > 0
                    && self
                        .completed_pomodoros
                        .is_multiple_of(self.config.long_break_interval)
                {
                    Phase::LongBreak
                } else {
                    Phase::ShortBreak
                };
                self.start_phase(next_phase);
            }
            Phase::ShortBreak | Phase::LongBreak => {
                self.start_phase(Phase::Work);
            }
            Phase::Idle => {}
        }
    }

    fn display_secs(&self) -> u32 {
        if self.phase == Phase::Idle {
            Phase::Work.total_secs(&self.config)
        } else {
            self.remaining_secs
        }
    }

    fn format_time(&self) -> String {
        // Round up so "0" only shows at exactly 0 seconds
        let mins = self.display_secs().div_ceil(60);
        format!("{mins}")
    }

    fn format_time_full(&self) -> String {
        let secs = self.display_secs();
        format!("{:02}:{:02}", secs / 60, secs % 60)
    }

    fn save_config(&self) {
        if let Some(ref handler) = self.config_handler {
            let _ = self.config.write_entry(handler);
        }
    }

    fn phase_color(&self) -> cosmic::iced::Color {
        match self.phase {
            // Red for work
            Phase::Work => cosmic::iced::Color::from_rgb(0.91, 0.30, 0.24),
            // Green for short break
            Phase::ShortBreak => cosmic::iced::Color::from_rgb(0.18, 0.80, 0.44),
            // Blue for long break
            Phase::LongBreak => cosmic::iced::Color::from_rgb(0.20, 0.60, 0.86),
            // Neutral for idle
            Phase::Idle => cosmic::iced::Color::from_rgba(1.0, 1.0, 1.0, 0.1),
        }
    }

    fn settings_section(&self) -> Element<'_, Message> {
        widget::column()
            .push(setting_row(
                fl!("work-mins"),
                self.config.work_mins,
                1,
                60,
                Message::SetWorkMins,
            ))
            .push(setting_row(
                fl!("short-break-mins"),
                self.config.short_break_mins,
                1,
                30,
                Message::SetShortBreakMins,
            ))
            .push(setting_row(
                fl!("long-break-mins"),
                self.config.long_break_mins,
                1,
                60,
                Message::SetLongBreakMins,
            ))
            .push(setting_row(
                fl!("long-break-interval"),
                self.config.long_break_interval,
                1,
                10,
                Message::SetLongBreakInterval,
            ))
            .spacing(8)
            .into()
    }

    fn phase_color_muted(&self) -> cosmic::iced::Color {
        let c = self.phase_color();
        cosmic::iced::Color::from_rgba(c.r, c.g, c.b, 0.3)
    }
}

fn colored_bg(color: cosmic::iced::Color, radius: f32) -> impl Fn(&Theme) -> container::Style {
    move |_theme: &Theme| container::Style {
        background: Some(color.into()),
        border: cosmic::iced::Border {
            radius: radius.into(),
            ..Default::default()
        },
        ..container::Style::default()
    }
}

fn setting_row<'a>(
    label: String,
    value: u32,
    min: u32,
    max: u32,
    on_change: fn(u32) -> Message,
) -> Element<'a, Message> {
    widget::row()
        .push(widget::text(label).width(Length::Fill))
        .push(widget::spin_button(
            format!("{value}"),
            format!("{value}"),
            value,
            1,
            min,
            max,
            on_change,
        ))
        .align_y(Alignment::Center)
        .spacing(8)
        .into()
}

impl cosmic::Application for AppModel {
    type Executor = cosmic::executor::Default;
    type Flags = ();
    type Message = Message;

    const APP_ID: &'static str = "com.github.bgub.CosmicExtAppletPomodoro";

    fn core(&self) -> &cosmic::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut cosmic::Core {
        &mut self.core
    }

    fn init(
        core: cosmic::Core,
        _flags: Self::Flags,
    ) -> (Self, Task<cosmic::Action<Self::Message>>) {
        let (config, config_handler) =
            match cosmic_config::Config::new(Self::APP_ID, Config::VERSION) {
                Ok(handler) => {
                    let config = match Config::get_entry(&handler) {
                        Ok(config) => config,
                        Err((_errors, config)) => config,
                    };
                    (config, Some(handler))
                }
                Err(_) => (Config::default(), None),
            };

        let app = AppModel {
            core,
            config,
            config_handler,
            ..Default::default()
        };

        (app, Task::none())
    }

    fn on_close_requested(&self, id: Id) -> Option<Message> {
        Some(Message::PopupClosed(id))
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let (_, panel_h) = self.core.applet.suggested_window_size();
        #[allow(clippy::cast_precision_loss)]
        let diameter = panel_h.get() as f32;
        let radius = diameter / 2.0;

        let icon_bytes = if self.paused { PAUSE_SVG } else { TOMATO_SVG };
        let icon = widget::icon(widget::icon::from_svg_bytes(icon_bytes).symbolic(true))
            .width(Length::Fixed(18.0))
            .height(Length::Fixed(18.0));

        let active = self.phase != Phase::Idle;
        let bg_color = if active {
            self.phase_color_muted()
        } else {
            cosmic::iced::Color::TRANSPARENT
        };

        let mut row = widget::row().push(icon).align_y(Alignment::Center);
        if active {
            row = row
                .push(widget::text(self.format_time()).size(14.0))
                .spacing(4);
        }

        let pill_height = diameter * 0.8;
        let pill_radius = pill_height / 2.0;
        let content = widget::container(row)
            .height(Length::Fixed(if active { pill_height } else { diameter }))
            .width(if active {
                Length::Shrink
            } else {
                Length::Fixed(diameter)
            })
            .align_x(Alignment::Center)
            .align_y(Alignment::Center)
            .padding(if active {
                [0.0, pill_radius / 2.0]
            } else {
                [0.0, 0.0]
            })
            .style(move |_theme: &Theme| {
                let r = if active { pill_radius } else { radius };
                container::Style {
                    background: Some(bg_color.into()),
                    border: cosmic::iced::Border {
                        radius: r.into(),
                        ..Default::default()
                    },
                    ..container::Style::default()
                }
            });

        let btn = widget::button::custom(self.core.applet.autosize_window(content))
            .on_press(Message::ToggleTimer)
            .class(cosmic::theme::Button::AppletIcon);

        widget::mouse_area(btn)
            .on_right_release(Message::TogglePopup)
            .into()
    }

    fn view_window(&self, _id: Id) -> Element<'_, Self::Message> {
        let phase_text = match self.phase {
            Phase::Idle => fl!("idle"),
            Phase::Work => fl!("work"),
            Phase::ShortBreak => fl!("short-break"),
            Phase::LongBreak => fl!("long-break"),
        };

        let timer_text = self.format_time_full();

        let progress = self.progress();
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let bar_width = (280.0 * progress) as u16;

        let progress_bar = widget::container(
            widget::container(widget::space())
                .width(Length::Fixed(f32::from(bar_width)))
                .height(6)
                .style(colored_bg(self.phase_color(), 4.0)),
        )
        .width(280)
        .height(6)
        .style(colored_bg(
            cosmic::iced::Color::from_rgba(1.0, 1.0, 1.0, 0.08),
            4.0,
        ));

        // Pomodoro dots — filled for completed, empty for remaining
        let mut dots = widget::row().spacing(6);
        let goal = self.config.long_break_interval.max(1);
        let completed_in_cycle = self.completed_in_cycle(goal);
        for i in 0..goal {
            let dot_color = if i < completed_in_cycle {
                self.phase_color()
            } else {
                cosmic::iced::Color::from_rgba(1.0, 1.0, 1.0, 0.15)
            };
            dots = dots.push(
                widget::container(widget::space())
                    .width(10)
                    .height(10)
                    .style(colored_bg(dot_color, 5.0)),
            );
        }

        // Timer display with colored background
        let timer_block = widget::container(
            widget::column()
                .push(widget::text::heading(phase_text))
                .push(widget::text::title1(timer_text))
                .spacing(4)
                .align_x(Alignment::Center),
        )
        .width(Length::Fill)
        .padding([20, 24])
        .align_x(Alignment::Center)
        .style(colored_bg(self.phase_color_muted(), 12.0));

        let content = widget::column()
            .push(timer_block)
            .push(progress_bar)
            .push(dots)
            .push(self.controls())
            .push(widget::divider::horizontal::default())
            .push(self.settings_section())
            .spacing(12)
            .align_x(Alignment::Center)
            .padding(12);

        self.core.applet.popup_container(content).into()
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        let mut subs = vec![
            self.core()
                .watch_config::<Config>(Self::APP_ID)
                .map(|update| Message::UpdateConfig(update.config)),
        ];

        if self.phase != Phase::Idle && !self.paused {
            struct TimerTick;
            subs.push(Subscription::run_with(
                std::any::TypeId::of::<TimerTick>(),
                |_| {
                    cosmic::iced::stream::channel::<Message>(1, async |mut channel| {
                        loop {
                            tokio::time::sleep(Duration::from_secs(1)).await;
                            _ = channel.send(Message::Tick).await;
                        }
                    })
                },
            ));
        }

        Subscription::batch(subs)
    }

    fn update(&mut self, message: Self::Message) -> Task<cosmic::Action<Self::Message>> {
        match message {
            Message::ToggleTimer => match (self.phase, self.paused) {
                (Phase::Idle, _) => self.start_timer(),
                (_, false) => {
                    self.paused = true;
                }
                (_, true) => self.reset_timer(),
            },
            Message::Tick => {
                if self.remaining_secs > 0 {
                    self.remaining_secs -= 1;
                } else {
                    self.advance_phase();
                }
            }
            Message::Start => self.start_timer(),
            Message::Pause => {
                self.paused = true;
            }
            Message::Resume => {
                self.paused = false;
            }
            Message::Reset => self.reset_timer(),
            Message::Skip => {
                self.advance_phase();
            }
            Message::SetWorkMins(val) => self.update_config_value(val, |config, value| {
                config.work_mins = value;
            }),
            Message::SetShortBreakMins(val) => self.update_config_value(val, |config, value| {
                config.short_break_mins = value;
            }),
            Message::SetLongBreakMins(val) => self.update_config_value(val, |config, value| {
                config.long_break_mins = value;
            }),
            Message::SetLongBreakInterval(val) => self.update_config_value(val, |config, value| {
                config.long_break_interval = value;
            }),
            Message::UpdateConfig(config) => {
                self.config = config;
            }
            Message::TogglePopup => {
                return if let Some(p) = self.popup.take() {
                    destroy_popup(p)
                } else {
                    let new_id = Id::unique();
                    self.popup.replace(new_id);
                    let mut popup_settings = self.core.applet.get_popup_settings(
                        self.core.main_window_id().unwrap(),
                        new_id,
                        None,
                        None,
                        None,
                    );
                    popup_settings.positioner.size_limits = Limits::NONE
                        .max_width(372.0)
                        .min_width(300.0)
                        .min_height(100.0)
                        .max_height(400.0);
                    get_popup(popup_settings)
                };
            }
            Message::PopupClosed(id) => {
                if self.popup.as_ref() == Some(&id) {
                    self.popup = None;
                }
            }
        }
        Task::none()
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }
}
