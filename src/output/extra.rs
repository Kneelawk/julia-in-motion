use ffmpeg4::codec;
use ffmpeg4_sys::{av_opt_set, AV_OPT_SEARCH_CHILDREN};
use std::ffi::CString;

pub fn codec_opt_set_str(
    context: &mut codec::Context,
    name: &str,
    value: &str,
) -> Result<(), ffmpeg4::util::error::Error> {
    let name = CString::new(name).unwrap();
    let value = CString::new(value).unwrap();

    match unsafe {
        av_opt_set(
            context.as_mut_ptr() as *mut _,
            name.as_ptr(),
            value.as_ptr(),
            AV_OPT_SEARCH_CHILDREN,
        )
    } {
        0 => Ok(()),
        e => Err(ffmpeg4::util::error::Error::from(e)),
    }
}
