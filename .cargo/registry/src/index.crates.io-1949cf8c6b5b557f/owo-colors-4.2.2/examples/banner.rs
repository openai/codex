use owo_colors::{DynColors, OwoColorize};

const OWO: &str = r#"
                                     ██████╗ ██╗    ██╗ ██████╗ 
                                    ██╔═══██╗██║    ██║██╔═══██╗
                                    ██║   ██║██║ █╗ ██║██║   ██║
                                    ██║   ██║██║███╗██║██║   ██║
                                    ╚██████╔╝╚███╔███╔╝╚██████╔╝
                                     ╚═════╝  ╚══╝╚══╝  ╚═════╝
                                                                      
"#;

const COLORS: &str = r#"
                          .o88b. | .d88b.  |db      | .d88b.  |d8888b. |.d8888. 
                         d8P  Y8 |.8P  Y8. |88      |.8P  Y8. |88  `8D |88'  YP 
                         8P      |88    88 |88      |88    88 |88oobY' |`8bo.   
                         8b      |88    88 |88      |88    88 |88`8b   |  `Y8b. 
                         Y8b  d8 |`8b  d8' |88booo. |`8b  d8' |88 `88. |db   8D 
                          `Y88P' | `Y88P'  |Y88888P | `Y88P'  |88   YD |`8888Y' "#;

fn main() {
    let colors: [DynColors; 6] = [
        "#B80A41", "#4E4BA8", "#6EB122", "#DAAC06", "#00938A", "#E23838",
    ]
    .map(|color| color.parse().unwrap());

    println!("\n\n\n\n\n{}", OWO.fg_rgb::<0x2E, 0x31, 0x92>().bold());

    for line in COLORS.split_inclusive('\n') {
        for (text, color) in line.split('|').zip(colors.iter().copied()) {
            print!("{}", text.color(color).bold());
        }
    }

    println!("\n\n\n\n\n\n");
}
