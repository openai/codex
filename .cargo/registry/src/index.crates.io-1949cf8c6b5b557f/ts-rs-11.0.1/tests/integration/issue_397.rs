#[macro_export]
macro_rules! get_model_name {
    () => {{
        let mut module = module_path!().rsplit_once("::").unwrap().1.to_owned();
        module[0..1].make_ascii_uppercase();
        format!("{module}Model")
    }};
}

mod entities {
    mod users {
        use ts_rs::TS;

        #[derive(TS)]
        #[ts(export)]
        #[ts(export_to = "issue_397/")]
        #[ts(rename = {
            let mut module = module_path!().rsplit_once("::").unwrap().1.to_owned();
            module[0..1].make_ascii_uppercase();
            format!("{module}Model")
        })]
        struct Model;
    }

    mod posts {
        use ts_rs::TS;

        #[derive(TS)]
        #[ts(export)]
        #[ts(export_to = "issue_397/")]
        #[ts(rename = get_model_name!())]
        struct Model;
    }
}
