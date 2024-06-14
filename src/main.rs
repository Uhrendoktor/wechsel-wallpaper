use clap::{Parser, Subcommand};
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::{path::Path, process::Command};
use std::io::Write;

#[derive(Parser)]
#[command(version, about, long_about = None)]
#[command(propagate_version = true)]
#[command(arg_required_else_help = true)]
struct Args {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Initializes this wechsel plugin
    Init,

    /// Uninstalls this wechsel plugin
    DeInit {
        #[clap(long, short)]
        /// Deletes all wallpapers
        delete: bool,
    },

    /// Install wallpapers for the current or specified project
    Install { 
        /// Name of the wechsel project (default: current project)
        project: Option<String>,
        
        #[clap(long, short)]
        /// Path to the dark wallpaper
        dark: Option<String>,
        
        #[clap(long, short)]
        /// Path to the light wallpaper
        light: Option<String>,

        #[clap(long, short, default_value = "false")]
        /// replace existing wallpapers. This will delete the project's current wallpapers!
        replace: bool
    },
    /// Removes the wallpapers for the current or specified project
    Remove {
        /// Name of the wechsel project (default: current project)
        project: Option<String>,
    },
    /// Copies the wallpapers for the current or specified project to the specified path
    Save {
        /// Name of the wechsel project (default: current project)
        project: Option<String>,
        /// Path to save the wallpapers to
        path: String,
    }
}

pub fn get_wechsel_config_dir() -> PathBuf {
    let config_dir = dirs::config_dir().unwrap();
    config_dir.join("wechsel")
}

pub fn get_current_project() -> String {
    let wechsel_config_dir = get_wechsel_config_dir();
    let enviroment_variables = wechsel_config_dir.join("enviroment_variables.sh");
    let enviroment_variables = enviroment_variables.to_str().unwrap();

    String::from_utf8(Command::new("bash")
        .arg("-c")
        .arg(format!("source {enviroment_variables}; echo $PRJ"))
        .output()
        .expect("Unable to fetch current wechsel project. Please specify a project name.")
        .stdout
    ).expect("Unable to read project name from enviroment variables.").trim().to_string()
}

pub fn get_project_path(project: &str) -> PathBuf {
    PathBuf::from(String::from_utf8(Command::new("wechsel")
        .arg("get-path")
        .arg(project)
        .output()
        .expect(format!("Unable to fetch path for project: {}", project).as_str())
        .stdout
    ).expect("Unable to read project path from wechsel.").trim().to_string())
}

pub fn get_wallpaper_dir(project_path: &PathBuf) -> PathBuf {
    project_path.join(".wechsel_wallpapers")
}

pub fn get_project_wallpaper_dir(project: Option<String>) -> Result<PathBuf, String> {
    match project {
        Some(project) => {
            if project.is_empty() {
                return Err("Invalid Project name.".to_string());
            }
            let project_path = get_project_path(&project);
            if project_path.to_str().is_some_and(|str| str.is_empty()) {
                return Err(format!("Unable to fetch path for project: {}", project));
            }
            Ok(get_wallpaper_dir(&project_path))
        },
        None => {
            let project = get_current_project();
            if project.is_empty() {
                return Err("Unable to fetch current wechsel project. Please specify a project name.".to_string());
            }
            let project_path = get_project_path(&project);
            if project_path.to_str().is_some_and(|str| str.is_empty()) {
                return Err(format!("Unable to fetch path for project: {}", project));
            }
            Ok(get_wallpaper_dir(&project_path))
        }    
    }
}

pub fn get_default_wallpapers() -> (String, String) {
    fn get_gsettings_background_by_key(key: &str) -> String {
        String::from_utf8(Command::new("gsettings")
            .arg("get")
            .arg("org.gnome.desktop.background")
            .arg(key)
            .output()
            .expect(format!("Unable to fetch default {} wallpaper.", key).as_str())
            .stdout
        ).expect(format!("Unable to read default {} wallpaper.", key).as_str()).trim().to_string()
    }

    let default_light = get_gsettings_background_by_key("picture-uri");
    let default_dark = get_gsettings_background_by_key("picture-uri-dark");

    (default_light, default_dark)
}

fn main() {
    let args = Args::parse();

    match args.command {
        Some(Commands::Init) => {
            let wechsel_config_dir = get_wechsel_config_dir();

            let (default_light, default_dark) = get_default_wallpapers();

            fn create_and_write_file(path: &PathBuf, content: &str, mode: u32) {
                let path = path.to_str().unwrap();
                std::fs::OpenOptions::new()
                    .mode(mode)
                    .write(true)
                    .create(true)
                    .open(path)
                    .expect(format!("Unable to create {} file.", path).as_str())
                    .write_all(content.as_bytes())
                    .expect(format!("Unable to write to {} file.", path).as_str());
            }

            // create wechsel-wallpaper shell file
            let wechsel_wallpaper = wechsel_config_dir.join("wechsel-wallpaper");
            create_and_write_file(
                &wechsel_wallpaper,
                &format!(include_str!("../config_files/wechsel-wallpaper"), default_light=default_light, default_dark=default_dark),
                0o777
            );

            // create wechsel-wallpaper-deinit shell file
            let wechsel_wallpaper_deinit = wechsel_config_dir.join("wechsel-wallpaper-deinit");
            create_and_write_file(
                &wechsel_wallpaper_deinit, 
                &format!(include_str!("../config_files/wechsel-wallpaper-deinit"), default_light=default_light, default_dark=default_dark), 
                0o777
            );

            // create wechsel-wallpaper-installed file
            let wechsel_wallpaper_installed = wechsel_config_dir.join("wechsel-wallpaper-installed");
            create_and_write_file(&wechsel_wallpaper_installed, "", 0o666);
            
            // Append shell script at the end of wechsels on-prj-change script
            let on_prj_change = wechsel_config_dir.join("on-prj-change");
            let on_prj_change = on_prj_change.to_str().unwrap();
            std::fs::OpenOptions::new()
                .append(true)
                .open(on_prj_change)
                .expect("Unable to open on-prj-change file.")
                .write_all(include_str!("../config_files/wechsel-plugin").as_bytes())
                .expect("Unable to write to on-prj-change file.");

            println!("wechsel plugin [wechsel-wallpaper] initialized successfully.");
        },
        Some(Commands::DeInit {
            delete
        }) => {
            let wechsel_config_dir = get_wechsel_config_dir();

            let wechsel_wallpaper = wechsel_config_dir.join("wechsel-wallpaper");
            let wechsel_wallpaper = wechsel_wallpaper.to_str().unwrap();
            let _ = std::fs::remove_file(wechsel_wallpaper);

            let on_prj_change = wechsel_config_dir.join("on-prj-change");
            let on_prj_change = on_prj_change.to_str().unwrap();
            if let Ok(on_prj_change_content) = std::fs::read_to_string(on_prj_change) {
                let on_prj_change_content = on_prj_change_content.replace(include_str!("../config_files/wechsel-plugin"), "");
                let _ = std::fs::write(on_prj_change, on_prj_change_content);
            }

            // execute wechsel-wallpaper-deinit
            let wechsel_wallpaper_deinit = wechsel_config_dir.join("wechsel-wallpaper-deinit");
            let wechsel_wallpaper_deinit = wechsel_wallpaper_deinit.to_str().unwrap();
            Command::new("bash")
                .arg("-c")
                .arg(wechsel_wallpaper_deinit)
                .output()
                .expect("Unable to execute wechsel-wallpaper-deinit.");

            // delete all wallpapers
            if delete {
                let wechsel_wallpaper_installed = wechsel_config_dir.join("wechsel-wallpaper-installed");
                let wechsel_wallpaper_installed = wechsel_wallpaper_installed.to_str().unwrap();
                let projects = std::fs::read_to_string(wechsel_wallpaper_installed)
                    .expect("Unable to read wechsel-wallpaper-installed file.");
                for project in projects.lines() {
                    if let Ok(wallpaper_dir) = get_project_wallpaper_dir(Some(project.to_string())) {
                        if wallpaper_dir.exists() {
                            let _ = std::fs::remove_dir_all(&wallpaper_dir);
                        }
                    }
                }

                let _ = std::fs::remove_file(wechsel_wallpaper_installed);
            }

            println!("wechsel plugin [wechsel-wallpaper] deinitialized successfully.");
        }
        Some(Commands::Install { 
            project,
            dark,
            light,
            replace
        }) => {
            // Create wallpaper directory if it doesn't exist
            let wallpaper_dir = get_project_wallpaper_dir(project.clone()).unwrap_or_else(|err| panic!("{}", err));
            if !wallpaper_dir.exists() {
                std::fs::create_dir(&wallpaper_dir).expect("Unable to create wallpaper directory.");
            }

            // append the project name to the wechsel-wallpaper-installed file
            let wechsel_config_dir = get_wechsel_config_dir();
            let wechsel_wallpaper_installed = wechsel_config_dir.join("wechsel-wallpaper-installed");
            let wechsel_wallpaper_installed = wechsel_wallpaper_installed.to_str().unwrap();
            let project = project.unwrap_or_else(get_current_project);
            let _ = std::fs::OpenOptions::new()
                .append(true)
                .open(wechsel_wallpaper_installed)
                .expect("Unable to open wechsel-wallpaper-installed file.")
                .write_all(format!("{}\n", project).as_bytes())
                .expect("Unable to write to wechsel-wallpaper-installed file.");

            // check if dark and light wallpapers are provided and are valid paths
            if let Some(dark) = dark {
                let dark = Path::new(&dark);
                if !dark.exists() {
                    panic!("The entered dark wallpaper path is invalid.");
                }

                let dark_wallpaper = wallpaper_dir.join("dark.jpg");
                if dark_wallpaper.exists() && !replace {
                    panic!("Dark wallpaper already exists. Use --replace to overwrite. You may want to use `wechsel-wallpaper save` beforehand to not lose the current wallpapers. That would be tragic.");
                }

                std::fs::copy(dark, dark_wallpaper).expect("Unable to copy dark wallpaper.");
            }

            if let Some(light) = light {
                let light = Path::new(&light);
                if !light.exists() {
                    panic!("The entered light wallpaper path is invalid.");
                }

                let light_wallpaper = wallpaper_dir.join("light.jpg");
                if light_wallpaper.exists() && !replace {
                    panic!("Light wallpaper already exists. Use --replace to overwrite. You may want to use `wechsel-wallpaper save` beforehand to not lose the current wallpapers. That would be tragic.");
                }

                std::fs::copy(light, light_wallpaper).expect("Unable to copy light wallpaper.");
            }

            // run command
            Command::new("wechsel")
                .arg("change")
                .arg(project)
                .output()
                .expect("Unable to change wechsel project.");
                
            println!("Wallpapers installed successfully.");
        },
        Some(Commands::Remove { 
            project,
        }) => {
            let wallpaper_dir = get_project_wallpaper_dir(project.clone()).unwrap_or_else(|err| panic!("{}", err));
            if !wallpaper_dir.exists() {
                return;
            }

            std::fs::remove_dir_all(&wallpaper_dir).expect("Unable to delete wallpapers.");

            // remove the project name from the wechsel-wallpaper-installed file
            let wechsel_config_dir = get_wechsel_config_dir();
            let wechsel_wallpaper_installed = wechsel_config_dir.join("wechsel-wallpaper-installed");
            let wechsel_wallpaper_installed_content = std::fs::read_to_string(&wechsel_wallpaper_installed)
                .expect("Unable to read wechsel-wallpaper-installed file.")
                .replace(&project.unwrap_or_else(get_current_project), "");
            std::fs::write(wechsel_wallpaper_installed, wechsel_wallpaper_installed_content)
                .expect("Unable to write to wechsel-wallpaper-installed file.");

            println!("Wallpapers deleted successfully.");
        },
        Some(Commands::Save {
            project,
            path
        }) => {
            let project = project.unwrap_or_else(get_current_project);
            if project.is_empty() {
                panic!("Unable to fetch current wechsel project. Please specify a project name.");
            }

            let project_path = get_project_path(&project);
            if project_path.to_str().is_some_and(|str| str.is_empty()) {
                panic!("Unable to fetch path for project: {}", project);
            }

            let wallpaper_dir = get_wallpaper_dir(&project_path);
            if !wallpaper_dir.exists() {
                panic!("No wallpapers installed for project: {}", project);
            }

            let path = Path::new(&path);
            if !path.exists() {
                panic!("The entered path is invalid. The wallpapers cannot be saved there ):");
            }

            let dark_wallpaper = wallpaper_dir.join("dark.jpg");
            let light_wallpaper = wallpaper_dir.join("light.jpg");

            let dark_path = path.join("dark.jpg");
            let light_path = path.join("light.jpg");

            std::fs::copy(dark_wallpaper, dark_path).expect("Unable to copy dark wallpaper.");
            std::fs::copy(light_wallpaper, light_path).expect("Unable to copy light wallpaper.");

            println!("Wallpapers saved successfully.");
        },
        None => {}
    }
}