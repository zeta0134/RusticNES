use super::length_counter::LengthCounterState;
use super::audio_channel::AudioChannelState;
use super::audio_channel::PlaybackRate;
use super::audio_channel::Volume;
use super::audio_channel::Timbre;
use super::ring_buffer::RingBuffer;

pub struct TriangleChannelState {
    pub name: String,
    pub chip: String,
    pub debug_disable: bool,
    pub debug_buffer: Vec<i16>,
    pub output_buffer: RingBuffer,
    pub length_counter: LengthCounterState,

    pub control_flag: bool,
    pub linear_reload_flag: bool,
    pub linear_counter_initial: u8,
    pub linear_counter_current: u8,

    pub sequence_counter: u8,
    pub period_initial: u16,
    pub period_current: u16,
    pub length: u8,

    pub cpu_clock_rate: u64,
}

impl TriangleChannelState {
    pub fn new(channel_name: &str, chip_name: &str, cpu_clock_rate: u64) -> TriangleChannelState {
        return TriangleChannelState {
            name: String::from(channel_name),
            chip: String::from(chip_name),
            debug_disable: false,
            debug_buffer: vec!(0i16; 4096),
            output_buffer: RingBuffer::new(32768),
            length_counter: LengthCounterState::new(),
            control_flag: false,
            linear_reload_flag: false,
            linear_counter_initial: 0,
            linear_counter_current: 0,

            sequence_counter: 0,
            period_initial: 0,
            period_current: 0,
            length: 0,

            cpu_clock_rate: cpu_clock_rate,
        }
    }

    pub fn update_linear_counter(&mut self) {
        if self.linear_reload_flag {
            self.linear_counter_current = self.linear_counter_initial;
        } else {
            if self.linear_counter_current > 0 {
                self.linear_counter_current -= 1;
            }
        }
        if !(self.control_flag) {
            self.linear_reload_flag = false;
        }
    }

    pub fn clock(&mut self) {
        if self.linear_counter_current != 0 && self.length_counter.length > 0 {
            if self.period_current == 0 {
                // Reset the period timer, and clock the waveform generator
                self.period_current = self.period_initial;

                // The sequence counter starts at zero, but counts downwards, resulting in an odd
                // lookup sequence of 0, 7, 6, 5, 4, 3, 2, 1
                if self.sequence_counter >= 31 {
                    self.sequence_counter = 0;
                } else {
                    self.sequence_counter += 1;
                }
            } else {
                self.period_current -= 1;
            }
        }
    }

    pub fn output(&self) -> i16 {
        if self.period_initial <= 2 {
            // This frequency is so high that the hardware mixer can't keep up, and effectively
            // receives 7.5. We'll just return 7 here (close enough). Some games use this
            // to silence the channel, and returning 7 emulates the resulting clicks and pops.
            return 7;
        } else {
            let triangle_sequence = [0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,
                                     15,14,13,12,11,10,9,8,7,6,5,4,3,2,1,0];
            return triangle_sequence[self.sequence_counter as usize];
        }
    }
}

impl AudioChannelState for TriangleChannelState {
    fn name(&self) -> String {
        return self.name.clone();
    }

    fn chip(&self) -> String {
        return self.chip.clone();
    }

    fn sample_buffer(&self) -> &RingBuffer {
        return &self.output_buffer;
    }

    fn record_current_output(&mut self) {
        self.output_buffer.push(self.output());
    }

    fn min_sample(&self) -> i16 {
        return 0;
    }

    fn max_sample(&self) -> i16 {
        return 15;
    }

    fn muted(&self) -> bool {
        return self.debug_disable;
    }

    fn mute(&mut self) {
        self.debug_disable = true;
    }

    fn unmute(&mut self) {
        self.debug_disable = false;
    }

    fn playing(&self) -> bool {
        return 
            self.length_counter.length > 0 && 
            self.linear_counter_current != 0 &&
            self.period_initial > 2;
    }

    fn rate(&self) -> PlaybackRate {
        let frequency = self.cpu_clock_rate as f64 / (32.0 * (self.period_initial as f64 + 1.0));
        return PlaybackRate::FundamentalFrequency {frequency: frequency};
    }

    fn volume(&self) -> Option<Volume> {
        return None;
    }

    fn timbre(&self) -> Option<Timbre> {
        return None;
    }

    fn amplitude(&self) -> f64 {
        if self.playing() {
            return 0.55;
        }
        return 0.0;
    }
}