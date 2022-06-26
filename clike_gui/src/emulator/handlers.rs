use std::{time::Duration, sync::{Mutex, Arc}};

use eframe::epaint::{TextureHandle, ColorImage, Color32};
use mips_emulator::{cpu::{MipsCpu, CpuExternalHandler}, memory::page_pool::{MemoryDefaultAccess, MemoryDefault}};

use crate::util::keyboard_util::KeyboardMemory;


pub struct ExternalHandler{
    last_106: u128,
    rand_seed: u128,
    screen_texture: TextureHandle,
    keyboard: Arc<Mutex<KeyboardMemory>>,
    image: ColorImage,
    screen_x: usize,
    screen_y: usize,
}

impl ExternalHandler{
    fn opcode_address(cpu: &mut MipsCpu) -> u32{
        cpu.pc.wrapping_sub(4)
    }

    fn opcode(cpu: &mut MipsCpu) -> u32{
        cpu.mem.get_u32_alligned(cpu.pc.wrapping_sub(4))
    }

    pub fn new(screen_texture: TextureHandle, keyboard: Arc<Mutex<KeyboardMemory>>) -> Self {
        let time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
        Self {
            screen_texture,
            image: ColorImage::new([0,0], Color32::BLACK), 
            keyboard,
            screen_x: 0,
            screen_y: 0, 
            last_106: time,
            rand_seed: time,
        }
    }
}

impl CpuExternalHandler for ExternalHandler {
    fn arithmetic_error(&mut self, cpu: &mut MipsCpu, error_id:  u32) {
        log::warn!("arithmetic error {}", error_id);
        cpu.stop();
    }

    fn memory_error(&mut self, cpu: &mut MipsCpu, error_id: u32) {
        log::warn!("Memory Error: {}", error_id);
        cpu.stop();
    }

    fn invalid_opcode(&mut self, cpu: &mut MipsCpu) {            
        log::warn!("invalid opcode {:#08X} at {:#08X}", Self::opcode(cpu), Self::opcode_address(cpu));
        cpu.stop();
    }

    fn system_call(&mut self, cpu: &mut MipsCpu, call_id: u32) {
        match call_id {
            0 => cpu.stop(),
            1 => log::info!("{}", cpu.reg[4] as i32),
            4 => {
                let start_address = cpu.reg[4];
                let end_address = {
                    let mut i = start_address;
                    loop {
                        if let Option::Some(val) = cpu.mem.get_u8_o(i){
                            if val == 0{
                                break i;
                            }
                        }else{
                            break i;
                        }
                        if let Option::Some(val) = i.checked_add(1){
                            i = val;
                        }else{
                            break i;
                        }
                        if i - start_address >= 500{
                            break i
                        }
                    }
                };
                if start_address < end_address{
                    unsafe{
                        let ptr = cpu.mem.get_or_make_mut_ptr_to_address(start_address);
                        let slice = core::slice::from_raw_parts(ptr, (end_address - start_address) as usize);
                        match std::str::from_utf8(slice){
                            Ok(str) => {
                                log::info!("{}", str)
                            },
                            Err(_err) => {
                                log::info!("Malformed String");
                                cpu.stop();
                            },
                        }
                    }
                }
            }
            5 => {
                let mut string = String::new();
                let _ = std::io::stdin().read_line(&mut string);
                match string.parse::<i32>() {
                    Ok(val) => cpu.reg[2] = val as u32,
                    Err(_) => match string.parse::<u32>() {
                        Ok(val) => cpu.reg[2] = val,
                        Err(_) => {
                            self.system_call_error(cpu, 
                                call_id,
                                0,
                                "unable to parse integer".into(),
                            );
                        }
                    },
                }
            }
            99 => {
                let mut x = self.rand_seed as u32;
                x = ((x >> 16) ^ x).wrapping_mul(0x45d9f3bu32);
                x = ((x >> 16) ^ x).wrapping_mul(0x45d9f3bu32);
                x = (x >> 16) ^ x;
                let x = (x >> 1) as i32;
                

                let dif = (cpu.reg[5] as i32).wrapping_sub(cpu.reg[4] as i32);
                if dif > 0{
                    cpu.reg[2] = ((x % dif).wrapping_add(cpu.reg[4] as i32)) as u32;
                }else{
                    cpu.reg[2] = 0;
                }
                self.rand_seed = self.rand_seed.wrapping_add(1);
            }
            101 => match char::from_u32(cpu.reg[4]) {
                Some(val) => log::info!("{}", val),
                None => log::warn!("Invalid char{}", cpu.reg[4]),
            },
            102 => {
                let mut string = String::new();
                let _ = std::io::stdin().read_line(&mut string);
                string = string.replace("\n", "");
                string = string.replace("\r", "");
                if string.len() != 1 {
                    cpu.reg[2] = string.chars().next().unwrap() as u32;
                } else {
                    self.system_call_error(cpu, call_id, 0, "invalid input");
                }
            }
            104 => {
                if self.keyboard.lock().unwrap().is_pressed((cpu.reg[4] as u8 as char).to_ascii_uppercase()){
                    cpu.reg[2] = 1;
                }else{
                    cpu.reg[2] = 0;
                }
            }
            105 => {
                use std::thread;
                thread::sleep(Duration::from_millis(cpu.reg[4] as u64));
            }
            106 => {
                let time =
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis();
                let dur = time - self.last_106;
                
                if (cpu.reg[4]  as u128 ) >= dur{
                    std::thread::sleep(std::time::Duration::from_millis((cpu.reg[4] as u64) - (dur as u64)));
                    self.last_106 =
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis();
                    
                }else{
                    self.last_106 = time;
                }
                
            }
            107 => {
                cpu.reg[2] = (std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis()
                    & 0xFFFFFFFFu128) as u32;
            }
            130 => {
                cpu.reg[2] = (std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_micros()
                    & 0xFFFFFFFFu128) as u32;
            }
            111 => {
                cpu.stop();
            }
                 
            150 => {                
                self.screen_x = cpu.reg[4] as usize;
                self.screen_y = cpu.reg[5] as usize;
                self.image = ColorImage::new([self.screen_x, self.screen_y], Color32::BLACK)
            }
            151 => {
                self.image.pixels[(cpu.reg[4] + cpu.reg[5] * ((self.screen_x) as u32)) as usize] = u32_to_color32(cpu.reg[6]);
            }
            152 => {
                self.image.pixels[cpu.reg[4] as usize] = u32_to_color32(cpu.reg[5]);
            }
            153 => {                
                self.screen_texture.set(self.image.clone(), eframe::epaint::textures::TextureFilter::Nearest);
            }
            154 => {

            }
            155 => {
                let (h,s,v) = (cpu.reg[4] as f32, cpu.reg[5] as f32, cpu.reg[6] as f32);
                let (h ,s, v) = (h / 255.0, s / 255.0, v / 255.0);
                let (r,g,b) = eframe::egui::color::hsv_from_rgb([h, s, v]);
                let (r,g,b) = (r * 255.0, g * 255.0, b * 255.0);
                let (r,g,b) = (r as u32, g as u32, b as u32);
                let color = r | g << 8 | b << 16;
                cpu.reg[2] = color;
            }
            156 => {
                let color = u32_to_color32(cpu.reg[4]);
                for pixel in self.image.pixels.iter_mut(){
                    *pixel = color;
                }
            }
            _ => {
                self.system_call_error(cpu, call_id, 0, "invalid system call");
            }
        }
    }

    fn system_call_error(&mut self, cpu: &mut MipsCpu, call_id: u32, error_id: u32, message:  &str) {
        log::warn!(
            "System Call: {} Error: {} Message: {}",
            call_id,
            error_id,
            message
        );
        cpu.stop();
    }
}

fn u32_to_color32(num: u32) -> Color32{
    Color32::from_rgb(num as u8, (num >> 8) as u8 , (num >> 16) as u8)
}   