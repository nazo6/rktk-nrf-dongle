#![no_std]
#![no_main]
#![feature(impl_trait_in_assoc_type)]

use embassy_executor::Spawner;
use embassy_nrf::config::{HfclkSource, LfclkSource};
use embassy_nrf::gpio::Output;
use embassy_nrf::peripherals::USBD;
use embassy_nrf::usb::vbus_detect::SoftwareVbusDetect;
use embassy_nrf::{bind_interrupts, config::Config, peripherals::RADIO};
use embassy_nrf_esb::{prx::PrxRadio, RadioConfig};
use once_cell::sync::OnceCell;
use rktk::hooks::interface::dongle::DongleHooks;
use rktk_drivers_common::display::ssd1306::Ssd1306DisplayBuilder;
use rktk_drivers_common::usb::{CommonUsbDriverBuilder, UsbDriverConfig, UsbOpts};
use rktk_drivers_nrf::dongle::dongle::EsbDongleDrier;
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(pub struct Irqs {
    RADIO => embassy_nrf_esb::InterruptHandler<RADIO>;
    USBD => embassy_nrf::usb::InterruptHandler<USBD>;
    TWISPI0 => embassy_nrf::twim::InterruptHandler<embassy_nrf::peripherals::TWISPI0>;
});

static SOFTWARE_VBUS: OnceCell<SoftwareVbusDetect> = OnceCell::new();

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let mut config = Config::default();
    config.hfclk_source = HfclkSource::ExternalXtal;
    // config.lfclk_source = LfclkSource::ExternalXtal;
    let p = embassy_nrf::init(config);

    // let mut led = Output::new(
    //     p.P1_03,
    //     embassy_nrf::gpio::Level::Low,
    //     embassy_nrf::gpio::OutputDrive::Standard,
    // );

    defmt::info!("start");
    rktk::print!("start");
    let prx = PrxRadio::<'_, _, 64>::new(p.RADIO, Irqs, RadioConfig::default()).unwrap();
    let d = EsbDongleDrier::new(prx);

    let vbus = SOFTWARE_VBUS.get_or_init(|| SoftwareVbusDetect::new(true, true));
    let driver = embassy_nrf::usb::Driver::new(p.USBD, Irqs, vbus);
    let opts = UsbOpts {
        config: {
            let mut config = UsbDriverConfig::new(0xc0de, 0xcafe);

            config.manufacturer = Some("nazo6");
            config.product = Some("negL_dongle");
            config.serial_number = Some("12345623");
            config.max_power = 100;
            config.max_packet_size_0 = 64;
            config.supports_remote_wakeup = true;

            config
        },
        mouse_poll_interval: 1,
        kb_poll_interval: 5,
        driver,
    };
    let usb = CommonUsbDriverBuilder::new(opts);

    let display = Ssd1306DisplayBuilder::new(
        embassy_nrf::twim::Twim::new(
            p.TWISPI0,
            Irqs,
            p.P1_00,
            p.P0_11,
            rktk_drivers_nrf::display::ssd1306::recommended_i2c_config(),
        ),
        ssd1306::size::DisplaySize128x32,
    );

    rktk::task::dongle_start(usb, d, Hooks, Some(display)).await;
}

struct Hooks;
impl DongleHooks for Hooks {
    async fn on_dongle_data(
        &mut self,
        data: &mut rktk::drivers::interface::dongle::DongleData,
    ) -> bool {
        defmt::info!("{:?}", data);
        true
    }
}
