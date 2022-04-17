use std::{sync::{atomic::AtomicUsize}, time::Duration};

use crate::memory::{Memory, PagePoolRef, PagePoolListener, MemoryGuard, PagePoolController};

//macros
//jump encoding
macro_rules! jump_immediate_address {
    ($expr:expr) => {
        ($expr as u32) & 0b00000011111111111111111111111111
    };
}

macro_rules! jump_immediate_offset {
    ($expr:expr) => {
        (($expr as i32) << 6) >> 4
    };
}

//immediate encoding
macro_rules! immediate_immediate {
    ($expr:expr) => {
        (($expr as i32) << 16) >> 16
    };
}
macro_rules! immediate_immediate_address {
    ($expr:expr) => {
        (($expr as i32) << 16) >> 14
    };
}

macro_rules! immediate_immediate_unsigned {
    ($expr:expr) => {
        (($expr as u32) & 0xFFFF)
    };
}

macro_rules! immediate_immediate_unsigned_hi {
    ($expr:expr) => {
        (($expr as u32) << 16)
    };
}

macro_rules! immediate_s {
    ($expr:expr) => {
        ((($expr as u32) >> 21) & 0b11111) as usize
    };
}

macro_rules! immediate_t {
    ($expr:expr) => {
        ((($expr as u32) >> 16) & 0b11111) as usize
    };
}

macro_rules! register_s {
    ($expr:expr) => {
        ((($expr as u32) >> 21) & 0b11111) as usize
    };
}

macro_rules! register_t {
    ($expr:expr) => {
        ((($expr as u32) >> 16) & 0b11111) as usize
    };
}

macro_rules! register_d {
    ($expr:expr) => {
        ((($expr as u32) >> 11) & 0b11111) as usize
    };
}

macro_rules! register_a {
    ($expr:expr) => {
        (($expr as u32) >> 6) & 0b11111
    };
}
//Macros

#[derive(Default)]
pub struct MipsCpu {
    pub(crate) pc: u32,
    pub(crate) reg: [u32; 32],
    pub(crate) lo: u32,
    pub(crate) hi: u32,
    i_check: bool,
    running: bool,
    finished: bool,
    paused: AtomicUsize,
    is_paused: bool,
    is_within_memory_event: bool,
    pub(crate) mem: PagePoolRef<Memory>,
    #[cfg(feature = "external_handlers")]
    external_handler: CpuExternalHandler,
}

#[cfg(feature = "external_handlers")]
pub struct CpuExternalHandler {
    pub arithmetic_error: fn(&mut MipsCpu, u32),
    pub memory_error: fn(&mut MipsCpu, u32),
    pub invalid_opcode: fn(&mut MipsCpu),
    pub system_call: fn(&mut MipsCpu, u32),
    pub system_call_error: fn(&mut MipsCpu, u32, u32, &str),
}

#[cfg(feature = "external_handlers")]
impl Default for CpuExternalHandler {
    fn default() -> Self {
        fn f1(_a1: &mut MipsCpu, _a2: u32) {
            panic!("Unimplemented External Handler for CPU");
        }
        fn f2(_a1: &mut MipsCpu) {
            panic!("Unimplemented External Handler for CPU");
        }
        fn f3(_a1: &mut MipsCpu, _a2: u32, _a3: u32, _a4: &str) {
            panic!("Unimplemented External Handler for CPU");
        }

        Self {
            arithmetic_error: f1,
            memory_error: f1,
            system_call: f1,
            invalid_opcode: f2,
            system_call_error: f3,
        }
    }
}

impl PagePoolListener for MipsCpu{
    fn lock(&mut self, _initiator: bool) -> Result<(), Box<dyn std::error::Error>> {
        //if !initiator{
        println!("Paused!!!");
        self.pause_exclude_memory_event();
        //}
        Result::Ok(())
    }

    fn unlock(&mut self, _initiator: bool) -> Result<(), Box<dyn std::error::Error>> {
        //if !initiator{
        println!("Resumed!!!");
        self.resume();
        //}
        Result::Ok(())
    }
}

impl MipsCpu {
    #[allow(unused)]
    pub fn new() -> Self {
        let mut tmp = MipsCpu {
            pc: 0,
            reg: [0; 32],
            lo: 0,
            hi: 0,
            i_check: !false,
            running: false,
            finished: true,
            paused: 0.into(),
            is_paused: true,
            is_within_memory_event: false,
            mem: Memory::new(),
        };

        tmp
    }

    unsafe fn into_listener(&mut self) -> &'static mut (dyn PagePoolListener + Sync + Send){
        let test: &mut dyn PagePoolListener = self;
        std::mem::transmute(test)
    }

    #[allow(unused)]
    pub fn get_general_registers(&self) -> &[u32; 32] {
        &self.reg
    }
    #[allow(unused)]
    pub fn get_hi_register(&self) -> u32 {
        self.hi
    }
    #[allow(unused)]
    pub fn get_lo_register(&self) -> u32 {
        self.lo
    }
    #[allow(unused)]
    pub fn get_pc(&self) -> u32 {
        self.pc
    }
    //#[allow(unused)]
    //pub fn get_mem(&self) -> &Memory{
    //    &self.mem
    //}

    #[allow(unused)]
    pub fn get_general_registers_mut(&mut self) -> &mut [u32; 32] {
        &mut self.reg
    }
    #[allow(unused)]
    pub fn get_hi_register_mut(&mut self) -> &mut u32 {
        &mut self.hi
    }
    #[allow(unused)]
    pub fn get_lo_register_mut(&mut self) -> &mut u32 {
        &mut self.lo
    }
    #[allow(unused)]
    pub fn get_pc_mut(&mut self) -> &mut u32 {
        &mut self.pc
    }
    #[allow(unused)]
    pub fn get_mem(&mut self) -> MemoryGuard{
        self.mem.create_guard()
    }
    #[allow(unused)]
    pub fn get_mem_controller(&mut self) -> std::sync::Arc<std::sync::Mutex<PagePoolController>>{
        match &mut self.mem.page_pool{
            Some(val) => {
                val.clone_page_pool_mutex()
            },
            None => panic!(),
        }
    }

    #[allow(unused)]
    pub fn is_running(&self) -> bool {
        //this is a hack(dont come for me :))
        *crate::black_box(&self.running) || !*crate::black_box(&self.finished)
    }

    pub fn paused_or_stopped(&self) -> bool{
        self.is_paused() || !self.is_running()
    }

    pub fn is_paused(&self) -> bool {
        //this is a hack(dont come for me :))
        *crate::black_box(&self.is_paused)
    }

    fn is_within_memory_event(&self) -> bool {
        *crate::black_box(&self.is_within_memory_event)
    }

    #[allow(unused)]
    pub fn stop(&mut self) {
        self.running = false;
        self.i_check = !true;
    }

    #[allow(unused)]
    pub fn reset(&mut self) {
        self.pc = 0;
        self.reg = [0; 32];
        self.lo = 0;
        self.hi = 0;
    }

    #[allow(unused)]
    pub fn clear(&mut self) {
        self.reset();
        self.mem.unload_all_pages();
    }

    #[allow(unused)]
    pub fn pause(&mut self) {
        self.paused
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        while {
            //println!("CPU {:p}", self);
            self.i_check = !true;
            !self.is_paused()
        }{
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }

    #[allow(unused)]
    pub fn pause_exclude_memory_event(&mut self) {
        self.paused
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        while {
            //println!("CPU {:p}", self);
            self.i_check = !true;
            !(self.is_paused() || self.is_within_memory_event())
        }{
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }

    #[allow(unused)]
    pub fn resume(&mut self) {
        self.paused
            .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    }

    #[inline(always)]
    #[allow(unused)]
    fn system_call_error(&mut self, call_id: u32, error_id: u32, message: &str) {
        #[cfg(feature = "external_handlers")]
        {
            (self.external_handler.system_call_error)(self, call_id, error_id, message);
        }
        #[cfg(not(feature = "external_handlers"))]
        {
            log::warn!(
                "System Call: {} Error: {} Message: {}",
                call_id,
                error_id,
                message
            );
        }
    }

    #[inline(always)]
    fn memory_error(&mut self, error_id: u32) {
        #[cfg(feature = "external_handlers")]
        {
            (self.external_handler.memory_error)(self, error_id);
        }
        #[cfg(not(feature = "external_handlers"))]
        {
            log::warn!("Memory Error: {}", error_id);
        }
    }

    #[inline(always)]
    fn system_call(&mut self, call_id: u32) {
        #[cfg(feature = "external_handlers")]
        {
            (self.external_handler.system_call)(self, call_id);
        }
        #[cfg(not(feature = "external_handlers"))]
        {
            match call_id {
                0 => self.stop(),
                1 => log::info!("{}", self.reg[4] as i32),
                4 => {
                    let _address = self.reg[4];
                }
                5 => {
                    let mut string = String::new();
                    let _ = std::io::stdin().read_line(&mut string);
                    match string.parse::<i32>() {
                        Ok(val) => self.reg[2] = val as u32,
                        Err(_) => match string.parse::<u32>() {
                            Ok(val) => self.reg[2] = val,
                            Err(_) => {
                                self.system_call_error(
                                    call_id,
                                    0,
                                    "unable to parse integer".into(),
                                );
                            }
                        },
                    }
                }
                99 => {}
                101 => match char::from_u32(self.reg[4]) {
                    Some(val) => log::info!("{}", val),
                    None => log::warn!("Invalid char{}", self.reg[4]),
                },
                102 => {
                    let mut string = String::new();
                    let _ = std::io::stdin().read_line(&mut string);
                    string = string.replace("\n", "");
                    string = string.replace("\r", "");
                    if string.len() != 1 {
                        self.reg[2] = string.chars().next().unwrap() as u32;
                    } else {
                        self.system_call_error(call_id, 0, "invalid input");
                    }
                }
                105 => {
                    use std::thread;
                    thread::sleep(Duration::from_millis(self.reg[4] as u64));
                }
                106 => {
                    static mut LAST: std::time::SystemTime = std::time::UNIX_EPOCH;
                    let time = unsafe {
                        std::time::SystemTime::now()
                            .duration_since(LAST)
                            .unwrap()
                            .as_millis()
                    };
                    if time < self.reg[4] as u128 {
                        let time = self.reg[4] as u128 - time;
                        std::thread::sleep(std::time::Duration::from_millis(time as u64));
                    }
                }
                107 => {
                    self.reg[2] = (std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis()
                        & 0xFFFFFFFFu128) as u32;
                }
                108 => {
                    self.reg[2] = (std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_micros()
                        & 0xFFFFFFFFu128) as u32;
                }
                111 => {
                    self.stop();
                }
                _ => {}
            }
        }
    }

    #[inline(always)]
    fn arithmetic_error(&mut self, id: u32) {
        #[cfg(feature = "external_handlers")]
        {
            (self.external_handler.arithmetic_error)(self, id);
        }
        #[cfg(not(feature = "external_handlers"))]
        {
            log::warn!("arithmetic error {}", id);
        }
    }

    #[inline(always)]
    fn invalid_op_code(&mut self) {
        #[cfg(feature = "external_handlers")]
        {
            (self.external_handler.invalid_opcode)(self);
        }
        #[cfg(not(feature = "external_handlers"))]
        {
            log::warn!("invalid opcode {:#08X} at {:#08X}", self.mem.get_u32_alligned(self.pc.wrapping_sub(4)), self.pc.wrapping_sub(4));
            self.stop();
        }
    }

    #[allow(dead_code)]
    pub fn start_new_thread(&'static mut self) {
        if self.running || !self.finished {
            return;
        }

        self.running = true;
        self.finished = false;

        let _handle = std::thread::spawn(|| {
            // some work here
            log::info!("CPU Started");
            let start = std::time::SystemTime::now();
            self.run();
            let since_the_epoch = std::time::SystemTime::now()
                .duration_since(start)
                .expect("Time went backwards");
            log::info!("{:?}", since_the_epoch);
            log::info!("CPU stopping");
        });
    }

    #[allow(dead_code)]
    pub fn step_new_thread(&'static mut self) {
        if self.running || !self.finished {
            return;
        }
        self.running = false;
        self.finished = false;

        let _handle = std::thread::spawn(|| {
            log::info!("CPU Step Started");
            let start = std::time::SystemTime::now();
            self.run();
            let since_the_epoch = std::time::SystemTime::now()
                .duration_since(start)
                .expect("Time went backwards");
            log::info!("{:?}", since_the_epoch);
            log::info!("CPU Step Stopping");
        });
    }

    #[allow(dead_code)]
    pub fn start_local(&mut self) {
        if self.running || !self.finished {
            return;
        }

        self.running = true;
        self.finished = false;

        log::info!("CPU Step Started");
        let start = std::time::SystemTime::now();
        self.run();
        let since_the_epoch = std::time::SystemTime::now()
            .duration_since(start)
            .expect("Time went backwards");
        log::info!("{:?}", since_the_epoch);
        log::info!("CPU Step Stopping");
    }

    #[allow(arithmetic_overflow)]
    fn run(&mut self) {
        macro_rules! set_reg {
            ($reg:expr, $val:expr) => {
                unsafe {
                    *self.reg.get_unchecked_mut($reg) = $val;
                }
            };
        }
        macro_rules! get_reg {
            ($reg:expr) => {
                unsafe { *self.reg.get_unchecked($reg) }
            };
        }
        //TODO ensure that the memory isnt currently locked beforehand
        let listener = unsafe{self.into_listener()};
        self.mem.add_listener(listener);
        self.mem.add_thing(unsafe{std::mem::transmute(&mut self.is_within_memory_event)});

        self.is_paused = false;
        'main_loop: while {
            while *self.paused.get_mut() > 0 {
                self.is_paused = true;
                if !self.running {
                    break 'main_loop;
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            self.is_paused = false;

            //let mem = unsafe{&mut *(self.mem.deref_mut() as *mut Memory)};
            while self.i_check {
                let op =
                    self.mem.get_u32_alligned(self.pc);
                    //*[0x64027FFFu32, 0x00000820, 0x20210001, 0x10220001, 0x0BFFFFFD, 0x68000000].get_unchecked((self.pc >> 2) as usize)
                

                //prevent overflow
                self.pc = self.pc.wrapping_add(4);

                match op >> 26 {
                    0 => {
                        match op & 0b111111 {
                            // REGISTER formatted instructions

                            //arithmatic
                            0b100000 => {
                                //ADD
                                set_reg!(
                                    register_d!(op),
                                    ((self.reg[register_s!(op)] as i32)
                                        .wrapping_add(self.reg[register_t!((op))] as i32))
                                        as u32
                                );
                            }
                            #[allow(unreachable_patterns)]
                            0b100000 => {
                                //ADDU
                                self.reg[register_d!(op)] = self.reg[register_s!(op)]
                                    .wrapping_add(self.reg[register_t!((op))])
                            }
                            0b100100 => {
                                //AND
                                self.reg[register_d!(op)] =
                                    self.reg[register_s!(op)] & self.reg[register_t!((op))]
                            }
                            0b011010 => {
                                //DIV
                                let t = self.reg[register_d!(op)] as i32;
                                if t == 0 {
                                    let s = self.reg[register_s!(op)] as i32;
                                    self.lo = (s.wrapping_div(t)) as u32;
                                    self.hi = (s.wrapping_rem(t)) as u32;
                                } else {
                                    self.arithmetic_error(0);
                                }
                            }
                            0b011011 => {
                                //DIVU
                                let t = self.reg[register_d!(op)];
                                if t == 0 {
                                    let s = self.reg[register_s!(op)];
                                    self.lo = s.wrapping_div(t);
                                    self.hi = s.wrapping_rem(t);
                                } else {
                                    self.arithmetic_error(0);
                                }
                            }
                            0b011000 => {
                                //MULT
                                let t = self.reg[register_t!(op)] as i32 as i64;
                                let s = self.reg[register_s!(op)] as i32 as i64;
                                let result = t.wrapping_mul(s);
                                self.lo = (result & 0xFFFFFFFF) as u32;
                                self.hi = (result >> 32) as u32;
                            }
                            0b011001 => {
                                //MULTU
                                let t = self.reg[register_t!(op)] as u64;
                                let s = self.reg[register_s!(op)] as u64;
                                let result = t.wrapping_mul(s);
                                self.lo = (result & 0xFFFFFFFF) as u32;
                                self.hi = (result >> 32) as u32;
                            }
                            0b100111 => {
                                //NOR
                                self.reg[register_d!(op)] =
                                    (!(self.reg[register_s!(op)])) | self.reg[register_t!(op)];
                            }
                            0b100101 => {
                                //OR
                                self.reg[register_d!(op)] =
                                    self.reg[register_s!(op)] | self.reg[register_t!(op)];
                            }
                            0b100110 => {
                                //XOR
                                self.reg[register_d!(op)] =
                                    self.reg[register_s!(op)] ^ self.reg[register_t!(op)];
                            }
                            0b000000 => {
                                //SLL
                                self.reg[register_d!(op)] =
                                    self.reg[register_t!(op)] << register_a!(op);
                            }
                            0b000100 => {
                                //SLLV
                                self.reg[register_d!(op)] =
                                    self.reg[register_t!(op)] << self.reg[register_s!(op)];
                            }
                            0b000011 => {
                                //SRA
                                self.reg[register_d!(op)] =
                                    (self.reg[register_t!(op)] as i32 >> register_a!(op)) as u32;
                            }
                            0b000111 => {
                                //SRAV
                                self.reg[register_d!(op)] = (self.reg[register_t!(op)] as i32
                                    >> self.reg[register_s!(op)])
                                    as u32;
                            }
                            0b000010 => {
                                //SRL
                                self.reg[register_d!(op)] =
                                    self.reg[register_t!(op)] >> register_a!(op);
                            }
                            0b000110 => {
                                //SRLV
                                self.reg[register_d!(op)] =
                                    self.reg[register_t!(op)] >> self.reg[register_s!(op)];
                            }
                            0b100010 => {
                                //SUB
                                self.reg[register_d!(op)] = (self.reg[register_s!(op)] as i32
                                    - self.reg[register_t!(op)] as i32)
                                    as u32;
                            }
                            0b100011 => {
                                //SUBU
                                self.reg[register_d!(op)] =
                                    self.reg[register_s!(op)] - self.reg[register_t!(op)];
                            }

                            //comparason
                            0b101010 => {
                                //SLT
                                self.reg[register_d!(op)] = {
                                    if (self.reg[register_s!(op)] as i32)
                                        < (self.reg[register_t!(op)] as i32)
                                    {
                                        1
                                    } else {
                                        0
                                    }
                                }
                            }
                            0b101001 => {
                                //SLTU
                                self.reg[register_d!(op)] = {
                                    if self.reg[register_s!(op)] < self.reg[register_t!(op)] {
                                        1
                                    } else {
                                        0
                                    }
                                }
                            }

                            //jump
                            0b001001 => {
                                //JALR
                                self.reg[31] = self.pc;
                                self.pc = self.reg[register_s!(op)];
                            }
                            0b001000 => {
                                //JR
                                self.pc = self.reg[register_s!(op)];
                            }

                            //data movement
                            0b010000 => {
                                //MFHI
                                self.reg[register_d!(op)] = self.hi;
                            }
                            0b010010 => {
                                //MFLO
                                self.reg[register_d!(op)] = self.lo;
                            }
                            0b010001 => {
                                //MTHI
                                self.hi = self.reg[register_s!(op)];
                            }
                            0b010011 => {
                                //MTLO
                                self.lo = self.reg[register_s!(op)];
                            }
                            _ => self.invalid_op_code(),
                        }
                    }
                    //Jump formatted instruction
                    0b000010 => {
                        //jump
                        self.pc = (self.pc as i32 + jump_immediate_offset!(op)) as u32;
                    }
                    0b000011 => {
                        //jal
                        self.reg[31] = self.pc;
                        self.pc = (self.pc as i32 + jump_immediate_offset!(op)) as u32;
                    }
                    0b011010 => {
                        //trap
                        self.system_call(jump_immediate_address!(op));
                    }
                    // IMMEDIATE formmated instructions

                    // arthmetic
                    0b001000 => {
                        //trap ADDI
                        self.reg[immediate_t!(op)] = (self.reg[immediate_s!(op)] as i32
                            + immediate_immediate!(op) as i32)
                            as u32;
                    }
                    0b001001 => {
                        //trap ADDIU
                        self.reg[immediate_t!(op)] = self.reg[immediate_s!(op)] as u32
                            + immediate_immediate_unsigned!(op) as u32;
                    }
                    0b001100 => {
                        //trap ANDI
                        self.reg[immediate_t!(op)] = self.reg[immediate_s!(op)] as u32
                            & immediate_immediate_unsigned!(op) as u32
                    }
                    0b001101 => {
                        //trap ORI
                        self.reg[immediate_t!(op)] = self.reg[immediate_s!(op)] as u32
                            | immediate_immediate_unsigned!(op) as u32
                    }
                    0b001110 => {
                        //trap XORI
                        self.reg[immediate_t!(op)] = self.reg[immediate_s!(op)] as u32
                            ^ immediate_immediate_unsigned!(op) as u32
                    }
                    // constant manupulating inctructions
                    0b011001 => {
                        //LHI
                        let t = immediate_t!(op) as usize;
                        self.reg[t] =
                            self.reg[t] & 0xFFFF | immediate_immediate_unsigned_hi!(op) as u32;
                    }
                    0b011000 => {
                        //LLO
                        let t = immediate_t!(op) as usize;
                        self.reg[t] =
                            self.reg[t] & 0xFFFF0000 | immediate_immediate_unsigned!(op) as u32;
                    }

                    // comparison Instructions
                    0b001010 => {
                        //SLTI
                        self.reg[immediate_t!(op)] = {
                            if (self.reg[immediate_s!(op)] as i32)
                                < (immediate_immediate!(op) as i32)
                            {
                                1
                            } else {
                                0
                            }
                        }
                    }
                    #[allow(unreachable_patterns)]
                    0b001001 => {
                        //SLTIU
                        self.reg[immediate_t!(op)] = {
                            if (self.reg[immediate_s!(op) as usize] as u32)
                                < (immediate_immediate_unsigned!(op) as u32)
                            {
                                1
                            } else {
                                0
                            }
                        }
                    }

                    // branch instructions
                    0b000100 => {
                        //BEQ

                        if get_reg!(immediate_s!(op)) == get_reg!(immediate_t!(op)) {
                            self.pc = (self.pc as i32 + immediate_immediate_address!(op)) as u32;
                        }
                    }
                    0b000111 => {
                        //BGTZ
                        if self.reg[immediate_s!(op)] > 0 {
                            self.pc = (self.pc as i32 + immediate_immediate_address!(op)) as u32;
                        }
                    }
                    0b000110 => {
                        //BLEZ
                        if self.reg[immediate_s!(op)] <= 0 {
                            self.pc = (self.pc as i32 + immediate_immediate_address!(op)) as u32;
                        }
                    }
                    0b000101 => {
                        //BNE
                        if self.reg[immediate_s!(op)] != self.reg[immediate_t!(op) as usize] {
                            self.pc = (self.pc as i32 + immediate_immediate_address!(op)) as u32;
                        }
                    }

                    // load instrictions
                    0b100000 => {
                        //LB
                        self.reg[immediate_t!(op)] = self.mem.get_i8(
                            (self.reg[immediate_s!(op)] as i32 + immediate_immediate_address!(op))
                                as u32,
                        ) as u32
                    }
                    0b100100 => {
                        //LBU

                        self.reg[immediate_t!(op)] = self.mem.get_u8(
                            (self.reg[immediate_s!(op)] as i32 + immediate_immediate_address!(op))
                                as u32,
                        ) as u32
                    }
                    0b100001 => {
                        //LH
                        let address = (self.reg[immediate_s!(op)] as i32
                            + immediate_immediate_address!(op))
                            as u32;

                        #[cfg(feature = "memory_allignment_check")]
                        if address & 0b1 == 0 {
                            self.reg[immediate_t!(op)] = self.mem.get_i16_alligned(address) as u32
                        } else {
                            self.memory_error(0);
                        }
                        #[cfg(not(feature = "memory_allignment_check"))]
                        {
                            self.reg[immediate_t!(op)] = self.mem.get_i16_alligned(address) as u32
                        }
                    }
                    0b100101 => {
                        //LHU
                        let address = (self.reg[immediate_s!(op)] as i32
                            + immediate_immediate_address!(op))
                            as u32;

                        #[cfg(feature = "memory_allignment_check")]
                        if address & 0b1 == 0 {
                            self.reg[immediate_t!(op)] = self.mem.get_u16_alligned(address) as u32
                        } else {
                            self.memory_error(0);
                        }
                        #[cfg(not(feature = "memory_allignment_check"))]
                        {
                            self.reg[immediate_t!(op)] = self.mem.get_u16_alligned(address) as u32
                        }
                    }
                    0b100011 => {
                        //LW
                        let address = (self.reg[immediate_s!(op)] as i32
                            + immediate_immediate_address!(op))
                            as u32;

                        #[cfg(feature = "memory_allignment_check")]
                        if address & 0b11 == 0 {
                            self.reg[immediate_t!(op)] = self.mem.get_u32_alligned(address) as u32
                        } else {
                            self.memory_error(1);
                        }
                        #[cfg(not(feature = "memory_allignment_check"))]
                        {
                            self.reg[immediate_t!(op)] = self.mem.get_u32_alligned(address) as u32
                        }
                    }

                    // store instructions
                    0b101000 => {
                        //SB
                        self.mem.set_u8(
                            (self.reg[immediate_s!(op)] as i32 + immediate_immediate_address!(op))
                                as u32,
                            (self.reg[immediate_t!(op)] & 0xFF) as u8,
                        );
                    }
                    0b101001 => {
                        //SH
                        let address = (self.reg[immediate_s!(op)] as i32
                            + immediate_immediate_address!(op))
                            as u32;

                        if address & 0b11 == 0 {
                            self.mem.set_u16_alligned(
                                address,
                                (self.reg[immediate_t!(op)] & 0xFFFF) as u16,
                            );
                        } else {
                            self.memory_error(3);
                        }
                        #[cfg(not(feature = "memory_allignment_check"))]
                        {
                            self.mem.set_u16_alligned(
                                address,
                                (self.reg[immediate_t!(op)] & 0xFFFF) as u16,
                            );
                        }
                    }
                    0b101011 => {
                        //SW
                        let address = (self.reg[immediate_s!(op)] as i32
                            + immediate_immediate_address!(op))
                            as u32;
                        if address & 0b11 == 0 {
                            self.mem
                                .set_u32_alligned(address, (self.reg[immediate_t!(op)]) as u32);
                        } else {
                            self.memory_error(3);
                        }
                        #[cfg(not(feature = "memory_allignment_check"))]
                        {
                            self.mem
                                .set_u32_alligned(address, (self.reg[immediate_t!(op)]) as u32);
                        }
                    }

                    _ => self.invalid_op_code(),
                }
            }
            self.i_check = !false;

            //TODO clear paused state and check states when CPU stops running
            self.running //do while self.running
        } {}
        self.finished = true;
        self.mem.remove_listener();
        self.mem.remove_thing();
    }
}