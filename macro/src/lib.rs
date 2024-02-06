use proc_macro2::{Ident, Span, TokenStream, TokenTree};
use quote::quote;
use syn::Error;

/// Proc-macro to make construct SampleActionCard more convernient.
/// # Example
///
/// ```rust
/// // accept a const tuple as action button
/// const ACTION_BTN: (&str, &str) = ("btn_title", "btn_url");
///
/// // also accept function return tuple
/// fn DYNAMIC_BTN(p: &str) -> (&'static str, String) {
///     ("btn_title", format!("btn_url_dynamic_{p}"))
/// }
///
/// action_card! {
///     "Card Title",
///     format!("card text"),
///     [ACTION_BTN, (DYNAMIC_BTN("something"))]
/// }
/// ```
#[proc_macro]
pub fn action_card(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    try_expand(input, parse)
}

fn try_expand<P>(input: proc_macro::TokenStream, proc: P) -> proc_macro::TokenStream
where
    P: FnOnce(TokenStream) -> Result<TokenStream, Error>,
{
    match proc(TokenStream::from(input)) {
        Ok(tokens) => tokens,
        Err(err) => err.to_compile_error(),
    }
    .into()
}

fn parse(input: TokenStream) -> Result<TokenStream, Error> {
    let mut input = input.into_iter().peekable();
    let title = next(&mut input)?;
    let text = next(&mut input)?;

    let mut count = 0;
    let mut actions_expanded = Vec::new();
    let actions = next(&mut input)?;
    let TokenTree::Group(group) = actions
        .into_iter()
        .next()
        .ok_or_else(|| Error::new(Span::call_site(), "actions is empty"))?
    else {
        return Err(Error::new(Span::call_site(), "actions should inside []"));
    };

    for item in group.stream() {
        match item {
            TokenTree::Ident(..) | TokenTree::Group(..) => {
                count += 1;
                let title_key = Ident::new(&format!("action_title_{count}"), Span::call_site());
                let url_key = Ident::new(&format!("action_url_{count}"), Span::call_site());

                if let TokenTree::Ident(i) = item {
                    actions_expanded.push(quote! {
                        #title_key: #i.0.to_owned(),
                        #url_key: #i.1.to_owned(),
                    });
                } else if let TokenTree::Group(g) = item {
                    let tokens = g.stream();
                    actions_expanded.push(quote! {
                        #title_key: #tokens.0.to_owned(),
                        #url_key: #tokens.1.to_owned(),
                    });
                }
            }
            _ => {}
        }
    }

    let enum_name = Ident::new(&format!("SampleActionCard{count}"), Span::call_site());
    let quote = quote! {
        MessageTemplate::#enum_name {
            title: #(#title)*.to_owned(),
            text: #(#text)*.to_owned(),
            #(#actions_expanded)*
        }
    };

    Ok(quote)
}

fn next<I: Iterator<Item = TokenTree>>(i: &mut I) -> Result<Vec<TokenTree>, Error> {
    let mut result = vec![];
    loop {
        let Some(n) = i.next() else {
            break;
        };

        if let TokenTree::Punct(ref p) = n {
            if p.as_char() == ',' {
                break;
            }
        }

        result.push(n);
    }

    Ok(result)
}
