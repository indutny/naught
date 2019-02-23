extern crate wasmi;

use wasmi::{ImportsBuilder, Module, ModuleInstance, NopExternals};

use crate::error::Error;

pub struct Code {
    module: Module,
}

impl Code {
    pub fn from_blob(blob: &[u8]) -> Result<Self, Error> {
        let module = Module::from_buffer(&blob.to_owned())?;

        Ok(Code { module })
    }

    pub fn execute(&self) -> Result<Vec<u8>, Error> {
        let instance = ModuleInstance::new(&self.module, &ImportsBuilder::default())?;
        let instance = instance.run_start(&mut NopExternals)?;

        let value = instance.invoke_export("on_request", &[], &mut NopExternals)?;
        println!("{:#?}", value);

        Ok(vec![])
    }
}

#[cfg(test)]
mod tests {
    extern crate wabt;

    use super::*;

    #[test]
    fn it_should_execute() {
        let wasm_binary: Vec<u8> = wabt::wat2wasm(
            r#"
            (module
                (import "naught" "memory" (memory 1024))
                (func (export "on_request") (result i32)
                    i32.const 0
                    i32.store
                )
            )
            "#,
        )
        .expect("failed to parse wat");

        let code = Code::from_blob(&wasm_binary).expect("code to instantiate");

        code.execute().expect("to succeed");
    }
}
