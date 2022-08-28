use std::{error::Error, ptr::NonNull};

//use crate::{set_mem_alligned, get_mem_alligned, set_mem_alligned_o, get_mem_alligned_o};

use super::page_pool::{
    Page, PagePool, PagePoolController, PagePoolHolder, PagePoolListener, PagePoolNotifier,
    PagePoolRef, SEG_SIZE,
};

//stupid workaround
const INIT: Option<NonNull<Page>> = None;
pub struct Memory {
    pub(crate) listener: Option<&'static mut (dyn PagePoolListener + Send + Sync + 'static)>,
    pub(crate) page_pool: Option<PagePoolNotifier>,
    pub(crate) going_to_lock: Option<&'static mut bool>,
    pub(crate) page_table: [Option<NonNull<Page>>; SEG_SIZE],
}

unsafe impl Sync for Memory {}
unsafe impl Send for Memory {}

impl PagePoolHolder for Memory {
    fn init_holder(&mut self, notifier: PagePoolNotifier) {
        self.page_pool = Option::Some(notifier);
    }

    fn lock(&mut self, initiator: bool, _page_pool: &mut PagePool) -> Result<(), Box<dyn Error>> {
        match &mut self.listener {
            Some(val) => val.lock(initiator),
            None => Result::Ok(()),
        }
    }

    fn unlock(&mut self, initiator: bool, page_pool: &mut PagePool) -> Result<(), Box<dyn Error>> {
        for page in self.page_table.iter_mut() {
            *page = Option::None;
        }

        let pages = page_pool.pool.iter_mut();
        let mut addresses = page_pool.address_mapping.iter();
        for page in pages {
            self.page_table[(*addresses.next().unwrap()) as usize] = Option::Some(page.into());
        }

        match &mut self.listener {
            Some(val) => val.unlock(initiator),
            None => Result::Ok(()),
        }
    }

    fn get_notifier(&mut self) -> Option<&mut PagePoolNotifier> {
        self.page_pool.as_mut()
    }
}

impl Default for PagePoolRef<Memory> {
    fn default() -> Self {
        Memory::new()
    }
}

impl Memory {
    #[inline(never)]
    #[cold]
    unsafe fn create_page(&mut self, addr: u32) -> NonNull<Page> {
        let p = self.page_table.get_unchecked_mut(addr as usize >> 16);
        set_thing(&mut self.going_to_lock);
        match &self.page_pool {
            Some(val) => {
                let mut val = val.get_page_pool();
                let val = val.create_page((addr >> 16) as u16);
                if let Ok(ok) = val {
                    *p = Option::Some(ok);
                }
            }
            None => todo!(),
        }
        unset_thing(&mut self.going_to_lock);

        match p {
            Some(val) => *val,
            None => std::hint::unreachable_unchecked(),
        }
    }
}

impl<'a> super::page_pool::MemoryDefault<'a, NonNull<Page>> for Memory {
    #[inline(always)]
    unsafe fn get_page(&mut self, address: u32) -> Option<NonNull<Page>> {
        let addr = (address >> 16) as usize;
        *self.page_table.get_unchecked_mut(addr)
    }

    #[inline(never)]
    unsafe fn get_or_make_page(&mut self, address: u32) -> NonNull<Page> {
        //we don't need to check if the addr is in bounds since it is always below 2^16
        let addr = (address >> 16) as usize;

        match self.page_table.get_unchecked_mut(addr) {
            Some(val) => *val,
            None => self.create_page(address),
        }
    }
}

unsafe impl<'a> super::page_pool::MemoryDefaultAccess<'a, NonNull<Page>> for Memory {}

#[allow(dead_code)]
impl Memory {
    pub fn new() -> PagePoolRef<Self> {
        let controller = PagePoolController::new();
        let mut lock = controller.lock();
        match lock.as_mut() {
            Ok(lock) => {
                let mem = box Memory {
                    going_to_lock: Option::None,
                    page_pool: Option::None,
                    page_table: [INIT; SEG_SIZE],
                    listener: Option::None,
                };
                lock.add_holder(mem)
            }
            Err(err) => panic!("{err}"),
        }
    }

    pub fn add_thing(&mut self, thing: &'static mut bool) {
        self.going_to_lock = Option::Some(thing);
    }
    pub fn remove_thing(&mut self) {
        self.going_to_lock = Option::None;
    }

    pub fn add_listener(&mut self, listener: &'static mut (dyn PagePoolListener + Send + Sync)) {
        self.listener = Option::Some(listener);
    }
    pub fn remove_listener(&mut self) {
        self.listener = Option::None;
    }

    pub fn unload_page_at_address(&mut self, address: u32) {
        match &self.page_pool {
            Some(val) => {
                set_thing(&mut self.going_to_lock);
                let _ = val.get_page_pool().remove_page((address >> 16) as u16);
                unset_thing(&mut self.going_to_lock);
                self.page_table[(address >> 16) as usize] = Option::None;
            }
            None => todo!(),
        }
    }
    pub fn unload_all_pages(&mut self) {
        match &self.page_pool {
            Some(val) => {
                set_thing(&mut self.going_to_lock);
                let _ = val.get_page_pool().remove_all_pages();
                unset_thing(&mut self.going_to_lock);
                self.page_table.iter_mut().for_each(|page| {
                    *page = None;
                });
            }
            None => todo!(),
        }
    }
}

fn set_thing(thing: &mut Option<&'static mut bool>) {
    match thing {
        Some(some) => {
            **some = true;
        }
        None => {}
    }
}
fn unset_thing(thing: &mut Option<&'static mut bool>) {
    match thing {
        Some(some) => {
            **some = false;
        }
        None => {}
    }
}
