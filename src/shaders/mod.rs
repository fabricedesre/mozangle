#[allow(dead_code)]
#[allow(non_upper_case_globals)]
#[allow(non_camel_case_types)]
#[allow(non_snake_case)]
pub mod ffi {
    use std::os::raw::{c_char, c_int, c_uint, c_void};

    include!("bindings.rs");

    extern "C" {
        pub fn GLSLangInitialize() -> c_int;
        pub fn GLSLangFinalize() -> c_int;
        pub fn GLSLangInitBuiltInResources(res: *mut ShBuiltInResources);
        pub fn GLSLangConstructCompiler(
            _type: c_uint,
            spec: c_int,
            output: c_int,
            resources_ptr: *const ShBuiltInResources,
        ) -> ShHandle;
        pub fn GLSLangDestructCompiler(handle: ShHandle);
        pub fn GLSLangCompile(
            handle: ShHandle,
            strings: *const *const c_char,
            num_strings: usize,
            compile_options: ShCompileOptions,
        ) -> c_int;
        pub fn GLSLangClearResults(handle: ShHandle);
        pub fn GLSLangGetShaderVersion(handle: ShHandle) -> c_int;
        pub fn GLSLangGetShaderOutputType(handle: ShHandle) -> c_int;
        pub fn GLSLangGetObjectCode(handle: ShHandle) -> *const c_char;
        pub fn GLSLangGetInfoLog(handle: ShHandle) -> *const c_char;
        pub fn GLSLangIterUniformNameMapping(
            handle: ShHandle,
            each: unsafe extern "C" fn(*mut c_void, *const c_char, usize, *const c_char, usize),
            closure_each: *mut c_void,
        );
        pub fn GLSLangGetNumUnpackedVaryingVectors(handle: ShHandle) -> c_int;
    }
}

use self::ffi::ShShaderOutput::*;
use self::ffi::ShShaderSpec::*;
use self::ffi::*;

use std::collections::HashMap;
use std::default;
use std::ffi::CStr;
use std::ffi::CString;
use std::mem;
use std::os::raw::c_char;
use std::os::raw::c_void;
use std::slice;
use std::str;
use std::sync::Mutex;

lazy_static! {
    static ref CONSTRUCT_COMPILER_LOCK: Mutex<()> = Mutex::new(());
}

pub fn initialize() -> Result<(), &'static str> {
    if unsafe { GLSLangInitialize() } == 0 {
        Err("Couldn't initialize GLSLang")
    } else {
        Ok(())
    }
}

pub fn finalize() -> Result<(), &'static str> {
    if unsafe { GLSLangFinalize() } == 0 {
        Err("Couldn't finalize GLSLang")
    } else {
        Ok(())
    }
}

pub trait AsAngleEnum {
    fn as_angle_enum(&self) -> i32;
}

pub enum ShaderSpec {
    Gles2,
    WebGL,
    Gles3,
    WebGL2,
    WebGL3,
}

impl AsAngleEnum for ShaderSpec {
    #[inline]
    fn as_angle_enum(&self) -> i32 {
        (match *self {
            ShaderSpec::Gles2 => SH_GLES2_SPEC,
            ShaderSpec::WebGL => SH_WEBGL_SPEC,
            ShaderSpec::Gles3 => SH_GLES3_SPEC,
            ShaderSpec::WebGL2 => SH_WEBGL2_SPEC,
            ShaderSpec::WebGL3 => SH_WEBGL3_SPEC,
        }) as i32
    }
}

pub enum Output {
    Essl,
    Glsl,
    GlslCompat,
    GlslCore,
    Glsl130,
    Glsl140,
    Glsl150Core,
    Glsl330Core,
    Glsl400Core,
    Glsl410Core,
    Glsl420Core,
    Glsl430Core,
    Glsl440Core,
    Glsl450Core,
}

impl AsAngleEnum for Output {
    #[inline]
    fn as_angle_enum(&self) -> i32 {
        (match *self {
            Output::Essl => SH_ESSL_OUTPUT,
            Output::Glsl => SH_GLSL_COMPATIBILITY_OUTPUT,
            Output::GlslCompat => SH_GLSL_COMPATIBILITY_OUTPUT,
            Output::GlslCore => SH_GLSL_130_OUTPUT,
            Output::Glsl130 => SH_GLSL_130_OUTPUT,
            Output::Glsl140 => SH_GLSL_140_OUTPUT,
            Output::Glsl150Core => SH_GLSL_150_CORE_OUTPUT,
            Output::Glsl330Core => SH_GLSL_330_CORE_OUTPUT,
            Output::Glsl400Core => SH_GLSL_400_CORE_OUTPUT,
            Output::Glsl410Core => SH_GLSL_410_CORE_OUTPUT,
            Output::Glsl420Core => SH_GLSL_420_CORE_OUTPUT,
            Output::Glsl430Core => SH_GLSL_430_CORE_OUTPUT,
            Output::Glsl440Core => SH_GLSL_440_CORE_OUTPUT,
            Output::Glsl450Core => SH_GLSL_450_CORE_OUTPUT,
        }) as i32
    }
}

pub type BuiltInResources = ShBuiltInResources;

impl default::Default for BuiltInResources {
    fn default() -> BuiltInResources {
        unsafe {
            let mut ret: BuiltInResources = mem::zeroed();
            GLSLangInitBuiltInResources(&mut ret);
            ret
        }
    }
}

impl BuiltInResources {
    #[inline]
    pub fn empty() -> BuiltInResources {
        unsafe { mem::zeroed() }
    }
}

pub struct ShaderValidator {
    handle: ShHandle,
}

impl ShaderValidator {
    /// Create a new ShaderValidator instance
    /// NB: To call this you should have called first
    /// initialize()
    pub fn new(
        shader_type: u32,
        spec: ShaderSpec,
        output: Output,
        resources: &BuiltInResources,
    ) -> Option<ShaderValidator> {
        // GLSLangConstructCompiler is non-thread safe because it internally calls TCache::getType()
        // which writes/reads a std::map<T> with no locks.
        let _guard = CONSTRUCT_COMPILER_LOCK.lock().unwrap();
        let handle = unsafe {
            GLSLangConstructCompiler(
                shader_type,
                spec.as_angle_enum(),
                output.as_angle_enum(),
                resources,
            )
        };

        if handle.is_null() {
            return None;
        }

        Some(ShaderValidator { handle: handle })
    }

    #[inline]
    pub fn for_webgl(
        shader_type: u32,
        output: Output,
        resources: &BuiltInResources,
    ) -> Option<ShaderValidator> {
        Self::new(shader_type, ShaderSpec::WebGL, output, resources)
    }

    #[inline]
    pub fn for_webgl2(
        shader_type: u32,
        output: Output,
        resources: &BuiltInResources,
    ) -> Option<ShaderValidator> {
        Self::new(shader_type, ShaderSpec::WebGL2, output, resources)
    }

    pub fn compile(&self, strings: &[&str], options: ShCompileOptions) -> Result<(), &'static str> {
        let mut cstrings = Vec::with_capacity(strings.len());

        for s in strings.iter() {
            cstrings.push(CString::new(*s).map_err(|_| "Found invalid characters")?)
        }

        let cptrs: Vec<_> = cstrings.iter().map(|s| s.as_ptr()).collect();

        if unsafe {
            GLSLangCompile(
                self.handle,
                cptrs.as_ptr() as *const *const c_char,
                cstrings.len(),
                options,
            )
        } == 0
        {
            return Err("Couldn't compile shader");
        }
        Ok(())
    }

    pub fn object_code(&self) -> String {
        unsafe {
            let c_str = CStr::from_ptr(GLSLangGetObjectCode(self.handle));
            c_str.to_string_lossy().into_owned()
        }
    }

    pub fn info_log(&self) -> String {
        unsafe {
            let c_str = CStr::from_ptr(GLSLangGetInfoLog(self.handle));
            c_str.to_string_lossy().into_owned()
        }
    }

    pub fn compile_and_translate(&self, strings: &[&str]) -> Result<String, &'static str> {
        let options = SH_VALIDATE | SH_OBJECT_CODE |
                      SH_VARIABLES | // For uniform_name_map()
                      SH_EMULATE_ABS_INT_FUNCTION | // To workaround drivers
                      SH_EMULATE_ISNAN_FLOAT_FUNCTION | // To workaround drivers
                      SH_EMULATE_ATAN2_FLOAT_FUNCTION | // To workaround drivers
                      SH_CLAMP_INDIRECT_ARRAY_BOUNDS |
                      SH_INIT_GL_POSITION |
                      SH_ENFORCE_PACKING_RESTRICTIONS |
                      SH_LIMIT_EXPRESSION_COMPLEXITY |
                      SH_LIMIT_CALL_STACK_DEPTH;

        // Todo(Mortimer): Add SH_TIMING_RESTRICTIONS to options when the implementations gets better
        // Right now SH_TIMING_RESTRICTIONS is experimental
        // and doesn't support user callable functions in shaders

        self.compile(strings, options)?;
        Ok(self.object_code())
    }

    /// Returns a map from uniform name in the original shader to uniform name
    /// in the compiled shader.
    ///
    /// The map can be empty if the `SH_VARIABLES` option wasn't specified.
    pub fn uniform_name_map(&self) -> HashMap<String, String> {
        struct Closure {
            map: HashMap<String, String>,
            error: Option<str::Utf8Error>,
        }

        unsafe extern "C" fn each_c(
            closure: *mut c_void,
            first: *const c_char,
            first_len: usize,
            second: *const c_char,
            second_len: usize,
        ) {
            // Safety: code in or called from this function must not panic.
            // If it might and https://github.com/rust-lang/rust/issues/18510 is not fixed yet,
            // use std::panic::catch_unwind.
            let closure = closure as *mut Closure;
            let closure = &mut *closure;
            if closure.error.is_none() {
                macro_rules! to_string {
                    ($ptr: expr, $len: expr) => {
                        match str::from_utf8(slice::from_raw_parts($ptr as *const u8, $len)) {
                            Ok(s) => s.to_owned(),
                            Err(e) => {
                                closure.error = Some(e);
                                return;
                            }
                        }
                    };
                }
                closure
                    .map
                    .insert(to_string!(first, first_len), to_string!(second, second_len));
            }
        }

        let mut closure = Closure {
            map: HashMap::new(),
            error: None,
        };
        let closure_ptr: *mut Closure = &mut closure;
        unsafe { GLSLangIterUniformNameMapping(self.handle, each_c, closure_ptr as *mut c_void) }
        if let Some(err) = closure.error {
            panic!("Non-UTF-8 uniform name in ANGLE shader: {}", err)
        }
        closure.map
    }

    pub fn get_num_unpacked_varying_vectors(&self) -> i32 {
        unsafe { GLSLangGetNumUnpackedVaryingVectors(self.handle) }
    }
}

impl Drop for ShaderValidator {
    fn drop(&mut self) {
        unsafe { GLSLangDestructCompiler(self.handle) }
    }
}
