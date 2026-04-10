//! Optional SSD1306 OLED display over I²C.
//!
//! Wiring (defaults, change in `try_init` if your board is different):
//!   SDA → GPIO21
//!   SCL → GPIO22
//!   VCC → 3.3 V
//!   GND → GND
//!   I²C address: 0x3C (common), some modules ship 0x3D
//!
//! Strictly optional: `try_init` returns `None` on any failure and the
//! rest of the firmware holds `Option<OledDisplay>` so the same binary
//! boots headless on a bare chip and draws on a chip with a panel.
//!
//! 128×32 assumed. Change `DisplaySize128x32` to `DisplaySize128x64`
//! (and bump `LINE_COUNT` to 6) if you have the taller panel.

use anyhow::{anyhow, Result};
use embedded_graphics::{
    draw_target::DrawTarget,
    mono_font::{ascii::FONT_6X10, MonoTextStyle, MonoTextStyleBuilder},
    pixelcolor::BinaryColor,
    prelude::*,
    text::{Baseline, Text, TextStyleBuilder},
};
use esp_idf_svc::hal::gpio::{Gpio21, Gpio22};
use esp_idf_svc::hal::i2c::{I2cConfig, I2cDriver, I2C0};
use esp_idf_svc::hal::prelude::*;
use ssd1306::{
    mode::{BufferedGraphicsMode, DisplayConfig},  // DisplayConfig gives us init()
    prelude::{DisplayRotation, I2CInterface},     // I2CInterface re-exported from display-interface-i2c
    size::DisplaySize128x32,
    I2CDisplayInterface, Ssd1306,
};

/// Maximum lines we can fit at 6×10 font on a 128×32 panel.
pub const LINE_COUNT: usize = 3;

/// Concrete display type. Spelled out so we can name it in the
/// wrapper struct's field. The I²C interface type comes from
/// `display-interface-i2c`, which is what `I2CDisplayInterface::new`
/// returns under the hood in ssd1306 0.9.
pub type OledInner = Ssd1306<
    I2CInterface<I2cDriver<'static>>,
    DisplaySize128x32,
    BufferedGraphicsMode<DisplaySize128x32>,
>;

/// Thin wrapper around the initialized SSD1306 driver.
pub struct OledDisplay {
    inner: OledInner,
}

impl OledDisplay {
    /// Try to set up I²C + SSD1306 at address 0x3C. Returns `None` on
    /// any init failure (wrong wiring, no panel, bus error, etc.), with
    /// a WARN log breadcrumb so missing hardware is visible in the
    /// serial log.
    pub fn try_init(i2c: I2C0, sda: Gpio21, scl: Gpio22) -> Option<Self> {
        match init_inner(i2c, sda, scl) {
            Ok(inner) => {
                log::info!("display: SSD1306 128x32 initialized on I²C @ 0x3C");
                Some(Self { inner })
            }
            Err(e) => {
                log::warn!("display: SSD1306 init failed — running headless: {:#}", e);
                None
            }
        }
    }

    /// Render up to `LINE_COUNT` lines at 6×10 font, flush to the
    /// panel. Full framebuffer rewrite every time — trivially fast at
    /// 400 kHz I²C.
    pub fn show_lines(&mut self, lines: &[&str]) {
        // Clear via the DrawTarget trait method — this is the portable
        // way and matches what embedded-graphics examples do. Setting
        // the whole buffer to BinaryColor::Off (black) before drawing
        // the new frame.
        if let Err(e) = self.inner.clear(BinaryColor::Off) {
            log::warn!("display: clear failed: {:?}", e);
        }

        let style: MonoTextStyle<BinaryColor> = MonoTextStyleBuilder::new()
            .font(&FONT_6X10)
            .text_color(BinaryColor::On)
            .build();
        let text_style = TextStyleBuilder::new().baseline(Baseline::Top).build();

        // 10 px per line + 1 px gap. First baseline at y=0.
        for (i, line) in lines.iter().take(LINE_COUNT).enumerate() {
            let y = (i as i32) * 11;
            if let Err(e) =
                Text::with_text_style(line, Point::new(0, y), style, text_style)
                    .draw(&mut self.inner)
            {
                log::warn!("display: draw line {} failed: {:?}", i, e);
            }
        }

        if let Err(e) = self.inner.flush() {
            log::warn!("display: flush failed: {:?}", e);
        }
    }
}

/// Fallible init helper — all early errors map through one `?` chain
/// so `try_init` can turn them into a single `None` return.
fn init_inner(i2c: I2C0, sda: Gpio21, scl: Gpio22) -> Result<OledInner> {
    // 400 kHz is the standard "Fast mode" I²C rate every SSD1306
    // module supports. Higher is risky without short wires.
    let cfg = I2cConfig::new().baudrate(400.kHz().into());
    let driver = I2cDriver::new(i2c, sda, scl, &cfg)
        .map_err(|e| anyhow!("I2cDriver::new failed: {:?}", e))?;

    let iface = I2CDisplayInterface::new(driver);
    let mut disp = Ssd1306::new(iface, DisplaySize128x32, DisplayRotation::Rotate0)
        .into_buffered_graphics_mode();

    disp.init()
        .map_err(|e| anyhow!("SSD1306 init failed (is VCC wired? correct I²C addr?): {:?}", e))?;

    // clear_buffer() on BufferedGraphicsMode returns () — it only
    // touches the in-RAM buffer. No Result to propagate.
    disp.clear_buffer();

    disp.flush()
        .map_err(|e| anyhow!("SSD1306 flush failed: {:?}", e))?;
    Ok(disp)
}
