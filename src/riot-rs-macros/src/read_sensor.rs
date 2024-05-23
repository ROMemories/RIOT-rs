/// Reads a sensor from a sensor trait object.
///
/// # Panics
///
/// This macro panics when the `riot-rs` crate cannot be found as a dependency of the crate where
/// this macro is used.
#[proc_macro]
pub fn await_read_sensor_value(input: TokenStream) -> TokenStream {
    use quote::quote;
    use riot_rs_hwsetup::{sensors::{Sensor, StringOrTypePath}, HwSetup};
    use syn::Ident;

    let sensor_ident: Ident = syn::parse_macro_input!(input);

    let hwsetup_path = HwSetup::get_path_from_env().unwrap();
    let hwsetup = HwSetup::read_from_path(&hwsetup_path).unwrap();
    dbg!(&hwsetup);

    let sensor_type_list = hwsetup.sensors().iter().map(Sensor::driver);
    let sensor_type_list = sensor_type_list.map(|driver| {
        let driver = match StringOrTypePath::from(driver) {
            StringOrTypePath::TypePath(type_path) => type_path,
            _ => panic!("`driver` must start with an @"),
        };

        utils::parse_type_path(driver)
    });
    // FIXME: filter this type list based on context and enabled features

    let riot_rs_crate = utils::riot_rs_crate();

    // FIXME: we should generate the macro used by users in this macro, instead of doing the
    // opposite, so that the hw config file only gets parsed once

    // The `_read_sensor` macro expects a trailing comma
    let expanded = quote! {
        #riot_rs_crate::sensors::_await_read_sensor!(#sensor_ident, #(#sensor_type_list),* ,)
    };

    TokenStream::from(expanded)
}
