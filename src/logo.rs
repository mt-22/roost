pub const ROOST_LOGO: &str = r#"
                  ,.
                 (\(\)
 ,_              ;  o >
  {`-.          /  (_)
  `={\`-._____/`   |
   `-{ /    -=`\   |
    `={  -= = _/   /
       `\  .-'   /`               _ __ ___   ___  ___| |_
        {`-,__.'=                | '__/ _ \ / _ \/ __| __|
         ||                      | | | (_) | (_) \__ \ |_
         | \                     |_|  \___/ \___/|___/\__|
 --------/\/\---------------------------------------------
"#;

const TALKING_LOGO_HEAD: &str = r#"                  ,.
                 (\(\)
 ,_              ;  o >   "#;

const TALKING_LOGO_TAIL: &str = r#"
  {`-.          /  (_)
  `={\`-._____/`   |
   `-{ /    -=`\   |
    `={  -= = _/   /
       `\  .-'   /`               _ __ ___   ___  ___| |_
        {`-,__.'=                | '__/ _ \ / _ \/ __| __|
         ||                      | | | (_) | (_) \__ \ |_
         | \                     |_|  \___/ \___/|___/\__|
 --------/\/\---------------------------------------------
"#;

const GOODBYES: &[&str] = &[
    "bye bye",
    "l8r sk8r",
    "cya dude",
    "bye *b*tch",
    "until next time",
];

fn pick_random_goodbye() -> &'static str {
    let pid = std::process::id() as usize;
    GOODBYES[pid % GOODBYES.len()]
}

pub fn random_exit_banner() -> String {
    format!(
        "{}{}{}",
        TALKING_LOGO_HEAD,
        pick_random_goodbye(),
        TALKING_LOGO_TAIL
    )
}
