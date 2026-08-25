mod cycle;
mod pins;
mod registration;

use core::ffi::{c_char, c_int, c_long, c_void};
use core::{mem, ptr};

use dmc2_hal_sys as hal;

use self::cycle::update_component;
use self::pins::Component;
use self::registration::{publish_initial_values, register_interface};

const COMPONENT_NAME: &[u8] = b"h100_spindle\0";
const FUNCTION_NAME: &[u8] = b"h100-spindle\0";
const HAL_EXIT_FAILURE_MESSAGE: &[u8] = b"h100_spindle: ERROR: hal_exit() failed\n\0";
const ENOMEM: c_int = -12;
const EINVAL: c_int = -22;

static mut COMPONENT_ID: c_int = -1;

unsafe fn exit_component(component_id: c_int) {
    let result = unsafe { hal::hal_exit(component_id) };
    if result != 0 {
        unsafe {
            hal::rtapi_print_msg(
                hal::msg_level_t_RTAPI_MSG_ERR,
                HAL_EXIT_FAILURE_MESSAGE.as_ptr().cast::<c_char>(),
            );
        }
    }
}

#[cfg(not(debug_assertions))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    unsafe extern "C" {
        fn abort() -> !;
    }
    unsafe { abort() }
}

#[no_mangle]
pub extern "C" fn rtapi_app_main() -> c_int {
    let component_id = unsafe { hal::hal_init(COMPONENT_NAME.as_ptr().cast::<c_char>()) };
    if component_id <= 0 {
        return if component_id == 0 {
            EINVAL
        } else {
            component_id
        };
    }
    unsafe { COMPONENT_ID = component_id };

    let result = (|| -> Result<(), c_int> {
        let component =
            unsafe { hal::hal_malloc(mem::size_of::<Component>() as c_long) }.cast::<Component>();
        if component.is_null() {
            return Err(ENOMEM);
        }
        unsafe {
            ptr::write(component, Component::new());
            register_interface(component, component_id)?;
            publish_initial_values(&mut *component);
        }

        let exported = unsafe {
            hal::hal_export_funct(
                FUNCTION_NAME.as_ptr().cast::<c_char>(),
                Some(update_component),
                component.cast::<c_void>(),
                1,
                0,
                component_id,
            )
        };
        if exported != 0 {
            return Err(exported);
        }
        let ready = unsafe { hal::hal_ready(component_id) };
        if ready != 0 {
            return Err(ready);
        }
        Ok(())
    })();

    match result {
        Ok(()) => 0,
        Err(error) => {
            unsafe {
                exit_component(component_id);
                COMPONENT_ID = -1;
            }
            if error == 0 {
                EINVAL
            } else {
                error
            }
        }
    }
}

#[no_mangle]
pub extern "C" fn rtapi_app_exit() {
    let component_id = unsafe { COMPONENT_ID };
    if component_id >= 0 {
        unsafe {
            exit_component(component_id);
            COMPONENT_ID = -1;
        }
    }
}

#[cfg(test)]
mod tests;
