#[cfg(any(not(any(target_os = "windows", target_vendor = "apple")), test))]
use crate::Result;

#[cfg(any(not(any(target_os = "windows", target_vendor = "apple")), test))]
fn jni_uses_java_modified_utf8(env: &mut jni::Env<'_>) -> Result<bool> {
    match env
        .new_string("\u{10000}")?
        .mutf8_chars(env)?
        .to_bytes()
        .len()
    {
        6 => Ok(true),
        4 => Ok(false),
        _ => Err(jni::errors::Error::UnsupportedVersion)?,
    }
}

/// Detects whether the JNI runtime uses Java Modified UTF-8 and calls
/// [`configure_java_modified_utf8()`](crate::configure_java_modified_utf8) accordingly.
///
/// # Errors
/// Returns [`Error::JniError`](crate::Error::JniError) if the JNI calls fail.
#[cfg(not(any(target_os = "windows", target_vendor = "apple")))]
pub fn configure_java_modified_utf8_from_jni(env: &mut jni::Env<'_>) -> Result<()> {
    let uses_mutf8 = jni_uses_java_modified_utf8(env)?;
    crate::java_modified_utf8::configure_java_modified_utf8(uses_mutf8);
    Ok(())
}

// Tests require the JVM invocation API (libloading), available on unix/windows but not Android.
#[cfg(all(test, any(unix, windows), not(target_os = "android")))]
mod tests {
    use super::*;

    fn jvm() -> &'static jni::vm::JavaVM {
        use std::sync::OnceLock;
        static JVM: OnceLock<jni::vm::JavaVM> = OnceLock::new();
        JVM.get_or_init(|| {
            jni::vm::JavaVM::new(jni::InitArgsBuilder::new().build().unwrap())
                .expect("failed to create JVM")
        })
    }

    #[test]
    fn jni_detection_returns_bool_and_is_consistent() {
        jvm()
            .attach_current_thread(|env| -> jni::errors::Result<()> {
                let a = jni_uses_java_modified_utf8(env).unwrap();
                let b = jni_uses_java_modified_utf8(env).unwrap();
                assert_eq!(a, b);
                Ok(())
            })
            .unwrap();
    }

    // Standard JVMs (HotSpot, OpenJ9, etc.) always use Modified UTF-8 for
    // GetStringUTFChars. Modern Android would return false, but that cannot
    // be tested here since the JVM invocation API is not available on Android.
    #[test]
    fn jni_detection_returns_true() {
        jvm()
            .attach_current_thread(|env| -> jni::errors::Result<()> {
                assert!(jni_uses_java_modified_utf8(env).unwrap());
                Ok(())
            })
            .unwrap();
    }
}
