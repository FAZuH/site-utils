use dioxus::prelude::*;

#[component]
pub fn HeadEmbed(
    title: &'static str,
    description: &'static str,
    url: &'static str,
    image: &'static str,
) -> Element {
    rsx! {
        document::Meta { property: "og:title", content: title }
        document::Meta { property: "og:description", content: description }
        document::Meta { property: "og:url", content: url }
        document::Meta { property: "og:image", content: image }
        document::Meta { property: "og:type", content: "website" }
        document::Meta { name: "twitter:card", content: "summary_large_image" }
        document::Meta { name: "twitter:title", content: title }
        document::Meta { name: "twitter:description", content: description }
        document::Meta { name: "twitter:image", content: image }
    }
}
