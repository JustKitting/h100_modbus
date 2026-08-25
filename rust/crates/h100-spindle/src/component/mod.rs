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
const ENOMEM: c_int = hal::HalKnownErrno::OutOfMemory.raw();
#[cfg(test)]
const EINVAL: c_int = hal::HalKnownErrno::InvalidArgument.raw();

static mut COMPONENT_ID: c_int = -1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum StartupFailure {
    Hal(hal::HalError),
    Allocation,
}

impl StartupFailure {
    const fn safe_return_code(self) -> c_int {
        match self {
            Self::Hal(error) => error.safe_return_code(),
            Self::Allocation => ENOMEM,
        }
    }
}

impl From<hal::HalError> for StartupFailure {
    fn from(error: hal::HalError) -> Self {
        Self::Hal(error)
    }
}

unsafe fn exit_component(component_id: c_int) {
    if hal::HalCall::Exit
        .classify(unsafe { hal::hal_exit(component_id) })
        .is_err()
    {
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
    let component_id = match hal::HalCall::Init
        .classify(unsafe { hal::hal_init(COMPONENT_NAME.as_ptr().cast::<c_char>()) })
    {
        Ok(component_id) => component_id,
        Err(error) => return error.safe_return_code(),
    };
    unsafe { COMPONENT_ID = component_id };

    let result = (|| -> Result<(), StartupFailure> {
        let component =
            unsafe { hal::hal_malloc(mem::size_of::<Component>() as c_long) }.cast::<Component>();
        if component.is_null() {
            return Err(StartupFailure::Allocation);
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
        hal::HalCall::ExportFunct.classify(exported)?;
        hal::HalCall::Ready.classify(unsafe { hal::hal_ready(component_id) })?;
        Ok(())
    })();

    match result {
        Ok(()) => 0,
        Err(error) => {
            unsafe {
                exit_component(component_id);
                COMPONENT_ID = -1;
            }
            error.safe_return_code()
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
