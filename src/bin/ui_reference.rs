//! Static visual study of the user's Graphy reference. No capture or monitoring.
//! All coordinates below are reference-artboard/raster geometry, intentionally fixed
//! for comparison at 768x480 and uniformly scaled with the native canvas bounds.
use anyhow::{Context as _, Result};
use gpui_kit::{
    component::{Theme, ThemeMode},
    *,
};
use std::{borrow::Cow, time::Duration};

fn main() -> Result<()> {
    let mut timeout = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--close-after" => {
                timeout = Some(args.next().context("missing seconds")?.parse::<u64>()?)
            }
            "--help" => {
                println!(
                    "Static Graphy UI study. --close-after <seconds>. Escape / Alt+F4 closes. No real metrics."
                );
                return Ok(());
            }
            _ => anyhow::bail!("unknown argument {arg}"),
        }
    }
    gpui_kit::application().run(move |cx| {
        gpui_kit::init(cx);
        Theme::change(ThemeMode::Dark, None, cx);
        cx.text_system()
            .add_fonts(vec![
                Cow::Borrowed(include_bytes!("../../assets/fonts/Roboto-Regular.ttf")),
                Cow::Borrowed(include_bytes!("../../assets/fonts/Roboto-Bold.ttf")),
            ])
            .expect("embedded Roboto must load");
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                point(px(100.), px(100.)),
                size(px(960.), px(600.)),
            ))),
            titlebar: Some(TitlebarOptions {
                title: Some("NxMonitor · Graphy UI study · STATIC SAMPLE DATA".into()),
                ..Default::default()
            }),
            window_min_size: Some(size(px(768.), px(480.))),
            ..Default::default()
        };
        let (handle, _) = gpui_kit::open_window(options, cx, |window, cx| {
            cx.new(|cx| {
                let focus = cx.focus_handle();
                window.focus(&focus, cx);
                let close_task = timeout.map(|seconds| {
                    cx.spawn(async move |_, cx| {
                        smol::Timer::after(Duration::from_secs(seconds)).await;
                        cx.update(|cx| cx.quit());
                    })
                });
                Study {
                    focus,
                    _close_task: close_task,
                }
            })
        })
        .expect("open native visual study");
        handle
            .update(cx, |_, window, cx| {
                gpui_kit::base::Root::update(window, cx, |root, _, cx| {
                    root.style().background = Some(transparent_black().into());
                    cx.notify();
                });
            })
            .expect("configure visual study root");
        cx.on_window_closed(|cx, _| cx.quit()).detach();
    });
    Ok(())
}

struct Study {
    focus: FocusHandle,
    _close_task: Option<Task<()>>,
}
impl Render for Study {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .track_focus(&self.focus)
            .on_key_down(|event, _, cx| {
                if event.keystroke.key == "escape" {
                    cx.quit();
                }
            })
            .child(
                canvas(
                    |_, _, _| (),
                    |bounds, _, window, cx| {
                        let scale = (f32::from(bounds.size.width) / 768.)
                            .min(f32::from(bounds.size.height) / 480.);
                        let origin = point(
                            bounds.origin.x + (bounds.size.width - px(768. * scale)) / 2.,
                            bounds.origin.y + (bounds.size.height - px(480. * scale)) / 2.,
                        );
                        window.paint_quad(fill(bounds, Colors::reference().backdrop_low));
                        let mut p = Painter {
                            window,
                            cx,
                            origin,
                            scale,
                            colors: Colors::reference(),
                        };
                        p.backdrop();
                        p.text("CUSTOMIZABLE", 260., 18., 29., p.colors.white, true);
                        p.panel(88., 64., false);
                        p.panel(440., 64., true);
                        p.text(
                            "STATIC UI STUDY  /  SAMPLE DATA — NO MONITORING",
                            88.,
                            448.,
                            11.,
                            p.colors.caption,
                            false,
                        );
                        p.text(
                            "GPUI-Kit · native canvas",
                            537.,
                            448.,
                            11.,
                            p.colors.caption,
                            false,
                        );
                    },
                )
                .size_full(),
            )
    }
}

// Palette values live in one reference-theme definition, not in feature call sites.
struct Colors {
    white: Hsla,
    caption: Hsla,
    label: Hsla,
    green: Hsla,
    red: Hsla,
    yellow: Hsla,
    purple: Hsla,
    cyan: Hsla,
    pink: Hsla,
    panel: Hsla,
    dark: Hsla,
    backdrop: Hsla,
    backdrop_low: Hsla,
}
impl Colors {
    fn reference() -> Self {
        Self {
            white: rgb(0xe6edee).into(),
            caption: rgb(0x819396).into(),
            label: rgb(0x849799).into(),
            green: rgb(0xa5d73b).into(),
            red: rgb(0xd74836).into(),
            yellow: rgb(0xe1bf34).into(),
            purple: rgb(0xc35cd7).into(),
            cyan: rgb(0x58d4dd).into(),
            pink: rgb(0xe264a7).into(),
            panel: rgb(0x15282b).alpha(0.89).into(),
            dark: rgb(0x040b0c).into(),
            backdrop: rgb(0x173c38).into(),
            backdrop_low: rgb(0x040a0d).into(),
        }
    }
}
struct Painter<'a> {
    window: &'a mut Window,
    cx: &'a mut App,
    origin: Point<Pixels>,
    scale: f32,
    colors: Colors,
}
impl Painter<'_> {
    fn pos(&self, x: f32, y: f32) -> Point<Pixels> {
        self.origin + point(px(x * self.scale), px(y * self.scale))
    }
    fn bounds(&self, r: [f32; 4]) -> Bounds<Pixels> {
        Bounds::new(
            self.pos(r[0], r[1]),
            size(px(r[2] * self.scale), px(r[3] * self.scale)),
        )
    }
    fn rect(&mut self, r: [f32; 4], color: Hsla) {
        self.window.paint_quad(fill(self.bounds(r), color));
    }
    fn gradient(&mut self, r: [f32; 4], top: Hsla, bottom: Hsla) {
        self.window.paint_quad(fill(
            self.bounds(r),
            linear_gradient(
                180.,
                linear_color_stop(top, 0.),
                linear_color_stop(bottom, 1.),
            ),
        ));
    }
    fn line(&mut self, points: &[[f32; 2]], width: f32, color: Hsla) {
        if points.len() < 2 {
            return;
        }
        let mut path = PathBuilder::stroke(px(width * self.scale));
        path.move_to(self.pos(points[0][0], points[0][1]));
        for pt in &points[1..] {
            path.line_to(self.pos(pt[0], pt[1]));
        }
        self.window
            .paint_path(path.build().expect("finite reference path"), color);
    }
    fn text(
        &mut self,
        text: &'static str,
        x: f32,
        y: f32,
        font_size: f32,
        color: Hsla,
        bold: bool,
    ) {
        let font = Font {
            family: "Roboto".into(),
            weight: if bold {
                FontWeight::BOLD
            } else {
                FontWeight::NORMAL
            },
            ..Default::default()
        };
        let run = TextRun {
            len: text.len(),
            font,
            color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let line = self.window.text_system().shape_line(
            text.into(),
            px(font_size * self.scale),
            &[run],
            None,
        );
        line.paint(
            self.pos(x, y),
            px(font_size * 1.12 * self.scale),
            TextAlign::Left,
            None,
            self.window,
            self.cx,
        )
        .expect("paint embedded reference text");
    }
    fn right(&mut self, text: &'static str, edge: f32, y: f32, font_size: f32, color: Hsla) {
        let run = TextRun {
            len: text.len(),
            font: font("Roboto"),
            color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let line = self.window.text_system().shape_line(
            text.into(),
            px(font_size * self.scale),
            &[run],
            None,
        );
        let x = edge - f32::from(line.width) / self.scale;
        line.paint(
            self.pos(x, y),
            px(font_size * 1.12 * self.scale),
            TextAlign::Left,
            None,
            self.window,
            self.cx,
        )
        .expect("paint right-aligned text");
    }
    fn backdrop(&mut self) {
        self.gradient(
            [0., 0., 768., 480.],
            self.colors.backdrop,
            self.colors.backdrop_low,
        );
        // Abstract, code-native context only. The original game's scene is not a UI asset.
        for i in 0..110 {
            let x = 112. + i as f32 * 4.7;
            let n = noise(i, 11);
            self.gradient(
                [x, 15. + n * 4., 2.6, 53. + n * 22.],
                self.colors.green.opacity(0.32),
                self.colors.green.opacity(0.),
            );
        }
        self.line(
            &[[111., 26.], [629., 8.]],
            2.,
            self.colors.green.opacity(0.48),
        );
        self.line(
            &[[111., 96.], [628., 83.]],
            3.,
            self.colors.yellow.opacity(0.24),
        );
        self.gradient(
            [88., 372., 588., 45.],
            self.colors.purple.opacity(0.3),
            self.colors.purple.opacity(0.),
        );
        self.line(
            &[[86., 439.], [318., 444.], [559., 439.], [686., 450.]],
            3.,
            self.colors.yellow.opacity(0.43),
        );
        // Soft wide facets give the alpha panels a visible background without raster imagery.
        for i in 0..22 {
            let x = i as f32 * 38.;
            self.line(
                &[[x, 175.], [x + 28., 480.]],
                15.,
                self.colors.cyan.opacity(0.014),
            );
        }
    }
    fn panel(&mut self, x: f32, y: f32, cyan: bool) {
        let w = 232.;
        for spread in (1..=8).rev() {
            let s = spread as f32;
            self.rect(
                [x - s, y - s * 0.6, w + s * 2., 350. + s * 1.4],
                self.colors.dark.opacity(0.035),
            );
        }
        self.rect([x, y, w, 350.], self.colors.panel);
        let white = self.colors.white;
        let accent = if cyan {
            self.colors.cyan
        } else {
            self.colors.green
        };
        self.text("avg min max", x + 7., y + 4., 20., white, false);
        self.text(
            if cyan { "169" } else { "71" },
            x + 17.,
            y + 27.,
            21.,
            accent,
            false,
        );
        self.text(
            if cyan { "34" } else { "29" },
            x + 65.,
            y + 27.,
            21.,
            if cyan {
                self.colors.yellow
            } else {
                self.colors.red
            },
            false,
        );
        self.text(
            if cyan { "252" } else { "209" },
            x + 96.,
            y + 27.,
            21.,
            accent,
            false,
        );
        self.right(
            if cyan { "163 fps" } else { "59 fps" },
            x + w - 5.,
            y + 3.,
            25.,
            white,
        );
        self.right(
            if cyan { "6.12 ms" } else { "16.91 ms" },
            x + w - 5.,
            y + 31.,
            20.,
            white,
        );
        self.fps_chart(x + 4., y + 58., w - 8., 58., cyan);
        self.text("fps", x + 7., y + 110., 19., self.colors.label, false);
        let labels = ["reserved", "allocated", "mono"];
        let values = if cyan {
            ["527.2 MB", "351.5 MB", "16.4 MB"]
        } else {
            ["531.2 MB", "388.2 MB", "19.4 MB"]
        };
        let colors = if cyan {
            [self.colors.pink, self.colors.green, self.colors.cyan]
        } else {
            [self.colors.purple, self.colors.yellow, self.colors.cyan]
        };
        for i in 0..3 {
            let row_y = y + 144. + i as f32 * 19.;
            self.text(
                labels[i],
                x + if i == 2 { 36. } else { 7. },
                row_y,
                20.,
                white,
                false,
            );
            self.right(values[i], x + w - 5., row_y, 20., colors[i]);
        }
        self.memory_chart(x + 4., y + 205., w - 8., cyan);
        self.text("ram", x + 7., y + 235., 19., self.colors.label, false);
        self.right(
            if cyan { "-17.8 dB" } else { "-18.8 dB" },
            x + w - 5.,
            y + 266.,
            21.,
            white,
        );
        self.audio_chart(x + 4., y + 303., w - 8., cyan);
        self.text("audio", x + 7., y + 316., 20., self.colors.label, false);
    }
    fn fps_chart(&mut self, x: f32, y: f32, w: f32, h: f32, cyan: bool) {
        let accent = if cyan {
            self.colors.cyan
        } else {
            self.colors.green
        };
        for i in 0..112 {
            let n = noise(i, if cyan { 29 } else { 7 });
            let high = if cyan {
                12. + n * 17. + if i % 13 == 0 { 22. } else { 0. }
            } else if i % 9 == 0 || i % 17 == 0 {
                28. + n * 19.
            } else {
                3. + n * 3.
            };
            let xx = x + i as f32 * w / 112.;
            let yy = y + h - high;
            self.rect(
                [xx, yy, if cyan { 2.3 } else { 1.1 }, high],
                accent.opacity(if cyan { 0.1 } else { 0.07 }),
            );
            self.rect(
                [
                    xx,
                    yy,
                    if cyan { 2.5 } else { 1.2 },
                    if cyan { 2.4 } else { 1.4 },
                ],
                accent.opacity(0.95),
            );
        }
        self.line(
            &[
                [x, y + if cyan { 25. } else { 43. }],
                [x + w, y + if cyan { 25. } else { 43. }],
            ],
            1.2,
            self.colors.white.opacity(0.83),
        );
        self.line(
            &[[x, y + h - 3.], [x + w, y + h - 3.]],
            1.,
            self.colors.yellow.opacity(0.9),
        );
        self.line(
            &[[x, y + h + 4.], [x + w, y + h + 4.]],
            0.8,
            if cyan {
                self.colors.cyan.opacity(0.65)
            } else {
                self.colors.green.opacity(0.7)
            },
        );
        if cyan {
            self.line(
                &[[x, y + h - 12.], [x + w, y + h - 12.]],
                1.,
                self.colors.cyan.opacity(0.6),
            );
        }
    }
    fn memory_chart(&mut self, x: f32, y: f32, w: f32, cyan: bool) {
        let top = if cyan {
            self.colors.pink
        } else {
            self.colors.purple
        };
        self.gradient([x, y, w, 44.], top.opacity(0.24), top.opacity(0.));
        self.gradient(
            [x, y + 12., w, 32.],
            self.colors.cyan.opacity(0.09),
            self.colors.cyan.opacity(0.),
        );
        self.line(&[[x, y], [x + w, y]], 1., top.opacity(0.95));
        self.line(
            &[[x, y + 13.], [x + w, y + 13.]],
            0.9,
            if cyan {
                self.colors.green.opacity(0.8)
            } else {
                self.colors.yellow.opacity(0.45)
            },
        );
        self.line(
            &[[x, y + 43.], [x + w, y + 43.]],
            1.,
            self.colors.cyan.opacity(0.8),
        );
    }
    fn audio_chart(&mut self, x: f32, y: f32, w: f32, cyan: bool) {
        let mut peaks = Vec::new();
        for i in 0..36 {
            let f = i as f32 / 35.;
            let dip = if cyan {
                f * 17.
            } else {
                (f * 2.5).sin() * 10. + f * 6.
            };
            let envelope = 29. - dip + noise(i, 19) * 3.;
            let value = envelope * (0.38 + noise(i, 43) * 0.55);
            let xx = x + f * (w - 5.);
            self.gradient(
                [xx, y + 36. - value, 4.3, value],
                self.colors.white.opacity(0.28),
                self.colors.white.opacity(0.015),
            );
            self.rect(
                [xx, y + 36. - envelope, 4.4, 1.8],
                self.colors.white.opacity(0.95),
            );
            peaks.push([xx + 2., y + 36. - envelope]);
        }
        self.line(&peaks, 0.5, self.colors.white.opacity(0.18));
    }
}
fn noise(i: usize, seed: usize) -> f32 {
    ((i * 73 + seed * 29 + i * i * 17) % 101) as f32 / 100.
}
