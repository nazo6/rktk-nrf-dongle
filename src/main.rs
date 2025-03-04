#![no_std]
#![no_main]
#![feature(impl_trait_in_assoc_type)]

use embassy_executor::Spawner;
use embassy_nrf::{
    bind_interrupts,
    config::{Config, HfclkSource},
    peripherals::USBD,
    usb::vbus_detect::SoftwareVbusDetect,
};
use once_cell::sync::OnceCell;
use rktk::hooks::interface::dongle::default_dongle_hooks;
use rktk_drivers_common::{
    display::ssd1306::Ssd1306DisplayBuilder,
    usb::{CommonUsbDriverBuilder, UsbDriverConfig, UsbOpts},
};
use rktk_drivers_nrf::esb::{
    create_address,
    dongle::{EsbDongleDriverBuilder, EsbInterruptHandler, TimerInterruptHandler},
    Config as EsbConfig,
};
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(pub struct Irqs {
    RADIO => EsbInterruptHandler;
    TIMER0 => TimerInterruptHandler;
    USBD => embassy_nrf::usb::InterruptHandler<USBD>;
    TWISPI0 => embassy_nrf::twim::InterruptHandler<embassy_nrf::peripherals::TWISPI0>;
});

static SOFTWARE_VBUS: OnceCell<SoftwareVbusDetect> = OnceCell::new();

use embedded_alloc::LlffHeap as Heap;

#[global_allocator]
static HEAP: Heap = Heap::empty();

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let mut config = Config::default();
    config.hfclk_source = HfclkSource::ExternalXtal;
    // config.lfclk_source = LfclkSource::ExternalXtal;
    let p = embassy_nrf::init(config);

    use core::mem::MaybeUninit;
    const HEAP_SIZE: usize = 16384;
    static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];
    unsafe { HEAP.init(&raw mut HEAP_MEM as usize, HEAP_SIZE) }

    // let mut led = Output::new(
    //     p.P1_03,
    //     embassy_nrf::gpio::Level::Low,
    //     embassy_nrf::gpio::OutputDrive::Standard,
    // );

    defmt::info!("start");
    rktk::print!("start");
    let d = EsbDongleDriverBuilder::new(
        p.TIMER0,
        p.RADIO,
        Irqs,
        EsbConfig {
            addresses: create_address(90).unwrap(),
            ..Default::default()
        },
    );

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

    rktk::task::dongle_start(usb, d, default_dongle_hooks(), Some(display)).await;
}
