use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn machine(_attrs: TokenStream, fun: TokenStream) -> TokenStream {
    let f = syn::parse_macro_input!(fun as syn::ItemFn);

    assert!(
        f.attrs.is_empty(),
        "netsim_embed::machine cannot have any attribute"
    );
    assert!(
        f.sig.constness.is_none(),
        "netsim_embed::machine cannot be const"
    );
    assert!(
        f.sig.asyncness.is_none(),
        "netsim_embed::machine cannot be async"
    );
    assert!(
        f.sig.unsafety.is_none(),
        "netsim_embed::machine cannot be unsafe"
    );
    assert!(
        f.sig.abi.is_none(),
        "netsim_embed::machine cannot have an abi defined"
    );
    assert!(
        f.sig.generics.params.is_empty(),
        "netsim_embed::machine cannot be generic"
    );
    assert!(
        f.sig.inputs.len() == 1,
        "netsim_embed::machine must take exactly one argument"
    );
    assert!(
        f.sig.variadic.is_none(),
        "netsim_embed::machine cannot be variadic"
    );
    assert!(
        matches!(f.sig.output, syn::ReturnType::Default),
        "netsim_embed::machine must not declare a return type"
    );

    let input = match &f.sig.inputs.first().unwrap() {
        syn::FnArg::Typed(input) => input,
        _ => panic!("netsim_embed::machine must be a freestanding function"),
    };
    assert!(
        input.attrs.is_empty(),
        "netsim_embed::machine's only argument must not have any attributes attached"
    );

    let f_vis = f.vis;
    let f_ident = f.sig.ident;
    let input_ty = &input.ty;
    let id: u128 = rand::random();
    let input_pat = &input.pat;
    let f_block = f.block;

    TokenStream::from(quote::quote! {
        #[allow(non_camel_case_types)]
        #f_vis struct #f_ident ;

        impl netsim_embed::MachineFn for #f_ident {
            type Arg = #input_ty ;

            fn id() -> u128 {
                #id
            }

            fn call(#input_pat: #input_ty) #f_block
        }
    })
}
