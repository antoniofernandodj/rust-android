use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{
    parse_macro_input, AttributeArgs, FnArg, ItemFn, Lit, Meta, MetaNameValue, NestedMeta,
};

/// Convert snake_case to camelCase.
fn snake_to_camel(s: &str) -> String {
    let mut result = String::new();
    let mut next_upper = false;
    for ch in s.chars() {
        if ch == '_' {
            next_upper = true;
        } else if next_upper {
            result.extend(ch.to_uppercase());
            next_upper = false;
        } else {
            result.push(ch);
        }
    }
    result
}

/// `#[jni_call(package = "com.example.app", class = "Bridge")]`
///
/// Transforms:
/// ```rust,ignore
/// #[jni_call(package = "com.example.rustandroid", class = "Bridge")]
/// pub fn get_battery_level() -> i32 { 42 }
/// ```
/// into:
/// ```rust,ignore
/// #[no_mangle]
/// pub unsafe extern "system" fn Java_com_example_rustandroid_Bridge_getBatteryLevel() -> i32 {
///     std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| { 42 })).unwrap_or_default()
/// }
/// ```
#[proc_macro_attribute]
pub fn jni_call(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(attr as AttributeArgs);
    let func = parse_macro_input!(item as ItemFn);

    let mut package: Option<String> = None;
    let mut class: Option<String> = None;

    for arg in &args {
        if let NestedMeta::Meta(Meta::NameValue(MetaNameValue { path, lit, .. })) = arg {
            let ident = path.get_ident().map(|i| i.to_string()).unwrap_or_default();
            if let Lit::Str(s) = lit {
                match ident.as_str() {
                    "package" => package = Some(s.value()),
                    "class" => class = Some(s.value()),
                    _ => {}
                }
            }
        }
    }

    let package = package.expect("#[jni_call] requires `package = \"...\"`");
    let class = class.expect("#[jni_call] requires `class = \"...\"`");

    // com.example.foo  →  com_example_foo
    let package_ident = package.replace('.', "_");

    // get_battery_level  →  getBatteryLevel
    let rust_fn_name = func.sig.ident.to_string();
    let method_camel = snake_to_camel(&rust_fn_name);

    // Java_com_example_foo_Bridge_getBatteryLevel
    let jni_name = format!("Java_{}_{}_{}", package_ident, class, method_camel);
    let jni_ident = syn::Ident::new(&jni_name, Span::call_site());

    let vis = &func.vis;
    let inputs = &func.sig.inputs;
    let output = &func.sig.output;
    let body = &func.block;

    // Collect param names to forward the call (not needed since we inline the body)
    let _params: Vec<_> = inputs
        .iter()
        .filter_map(|arg| {
            if let FnArg::Typed(pt) = arg {
                Some(&pt.pat)
            } else {
                None
            }
        })
        .collect();

    let expanded = quote! {
        #[no_mangle]
        #vis unsafe extern "system" fn #jni_ident(#inputs) #output {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| #body)).unwrap_or_default()
        }
    };

    TokenStream::from(expanded)
}
