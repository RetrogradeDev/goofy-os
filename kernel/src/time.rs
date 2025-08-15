use x86_64::instructions::port::Port;

// Register Index	Value
// 0x00	Seconds     (0-59)
// 0x02	Minutes     (0-59)
// 0x04	Hours       (0-23)
// 0x07	Day of month (1-31)
// 0x08	Month       (1-12)
// 0x09	Year        (0-99)
// 0x32	Century     (e.g., 20)

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RtcTime {
    pub seconds: u8,
    pub minutes: u8,
    pub hours: u8,
    pub day: u8,
    pub month: u8,
    pub year: u16, // Full year, e.g. 2025
}

/// Reads the current time from the RTC.
fn read_rtc() -> RtcTime {
    while is_update_in_progress() {}

    let seconds = read_register(0x00);
    let minutes = read_register(0x02);
    let hours = read_register(0x04);
    let day = read_register(0x07);
    let month = read_register(0x08);
    let year = read_register(0x09);
    let century = read_register(0x32); // This register might not exist on all hardware

    // If an update started while we were reading, the values might be inconsistent.
    // In that case, we simply read again.
    if is_update_in_progress() {
        return read_rtc();
    }

    // Check the format (BCD or Binary) from Status Register B
    let register_b = read_register(0x0B);
    let is_bcd = (register_b & 0x04) == 0;

    if is_bcd {
        RtcTime {
            seconds: bcd_to_binary(seconds),
            minutes: bcd_to_binary(minutes),
            hours: bcd_to_binary(hours),
            day: bcd_to_binary(day),
            month: bcd_to_binary(month),
            year: (bcd_to_binary(century) as u16 * 100) + bcd_to_binary(year) as u16,
        }
    } else {
        RtcTime {
            seconds,
            minutes,
            hours,
            day,
            month,
            year: (century as u16 * 100) + year as u16,
        }
    }
}

fn read_register(reg: u8) -> u8 {
    unsafe {
        let mut command_port = Port::new(0x70);
        let mut data_port = Port::new(0x71);

        command_port.write(reg | 0x80);
        data_port.read()
    }
}

fn is_update_in_progress() -> bool {
    // Status Register A (0x0A), bit 7 (UIP) is set when an update is happening.
    (read_register(0x0A) & 0x80) != 0
}

fn bcd_to_binary(bcd_value: u8) -> u8 {
    (bcd_value & 0x0F) + ((bcd_value >> 4) * 10) // Magic :)
}

pub fn get_utc_time() -> RtcTime {
    read_rtc()
}

pub fn get_ms_since_epoch() -> i64 {
    let rtc_time = read_rtc();
    let year = rtc_time.year as i64;
    let month = rtc_time.month as i64;
    let day = rtc_time.day as i64;
    let hours = rtc_time.hours as i64;
    let minutes = rtc_time.minutes as i64;
    let seconds = rtc_time.seconds as i64;

    // Calculate the number of days since the epoch (1970-01-01)
    fn is_leap_year(year: i64) -> bool {
        (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
    }

    // Days in each month (non-leap year)
    const DAYS_IN_MONTH: [i64; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

    // Calculate days since epoch
    let mut days_since_epoch = 0;

    // Add days for each year since 1970
    for y in 1970..year {
        days_since_epoch += if is_leap_year(y) { 366 } else { 365 };
    }

    // Add days for each month in the current year
    for m in 0..(month - 1) {
        days_since_epoch += if m == 1 && is_leap_year(year) {
            29
        } else {
            DAYS_IN_MONTH[m as usize]
        };
    }

    // Add days in the current month
    days_since_epoch += day - 1;

    // Calculate the number of milliseconds since the epoch
    let ms_since_epoch = days_since_epoch * 24 * 60 * 60 * 1000
        + hours * 60 * 60 * 1000
        + minutes * 60 * 1000
        + seconds * 1000;

    ms_since_epoch
}
