/*
    Copyright (C) 2022  Kalka

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU Affero General Public License as
    published by the Free Software Foundation, either version 3 of the
    License, or (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU Affero General Public License for more details.

    You should have received a copy of the GNU Affero General Public License
    along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */
use std::collections::HashMap;
use rlua::{Function, Lua, Result, Table, UserDataMethods, Variadic};
use std::io::{Read, Write};
use colored::{Colorize};
use cumulus::{util, logger};

const LUA_VERSION: &str = "Lua 5.4.3";
const LUA_COPYRIGHT: &str = "  Copyright (C) 1994-2021 Lua.org, PUC-Rio";
const LUA_AUTHORS: &str = "R. Ierusalimschy, L. H. de Figueiredo, W. Celes";

fn main() -> Result<()> {
    logger::open_log_file_for_saving(None).unwrap();
    logger::set_virtual_terminal(true);

    util::attach_interrupt_handler(Some(|| {}));

    let args = std::env::args().collect::<Vec<String>>();
    let args_length = args.len();

    let lua = Lua::new();
    load_lua_log_library(&lua)?;
    load_color_library(&lua)?;
    load_http_library(&lua)?;
    // if 1st argument is a lua file, run it
    if args_length > 1 {
        let file_path = &args[1];
        if file_path.ends_with(".lua") {
            // If the file does not exist, exit
            if !std::path::Path::new(file_path).exists() {
                logger::error(&format!("File {} does not exist", file_path));
                std::process::exit(1);
            }

            lua.context(|lua_ctx| {
                // Open the file
                let file_stream = std::fs::File::open(file_path).unwrap();
                // Read the file
                let mut reader = std::io::BufReader::new(file_stream);
                // Read the file into a string
                let mut contents = String::new();
                reader.read_to_string(&mut contents).unwrap();
                let load_result = lua_ctx.load(&contents).exec();
                if load_result.is_err() {
                    logger::error(&format!(
                        "Failed to load file: {} [{}]",
                        file_path,
                        load_result.unwrap_err()
                    ));
                }
                // Check if the file has a main function
                // find in contents the string "function main"
                if contents.contains("function main") {
                    // Run the main function
                    let main_result = lua_ctx
                        .globals()
                        .get::<_, Function>("main")?
                        .call::<_, ()>(());
                    if main_result.is_err() {
                        logger::error(&format!(
                            "Failed to run main function in file: {} [{}]",
                            file_path,
                            main_result.unwrap_err()
                        ));
                    }
                }
                Ok(())
            })?;
        }
    }

    if args_length == 1 {
        println!(
            "{}",
            format!(
                "{}  {}\n{}",
                LUA_VERSION, LUA_COPYRIGHT, LUA_AUTHORS
            ).cyan().bold()
        );
        lua_interpret_loop(&lua)?;
    }

    Ok(())
}

#[tokio::main]
async fn get_http(url: &str) -> reqwest::Result<HashMap<String, String>> {
    let resp = reqwest::get(url).await?;
    let mut data = HashMap::new();
    if !resp.status().is_success() {
        data.insert("error".to_string(), resp.status().to_string());
        return Ok(data);
    }
    data.insert("status".to_string(), resp.status().to_string());
    data.insert("text".to_string(), resp.text().await?);

    Ok(data)
}

#[tokio::main]
async fn get_http_json(url: &str) -> reqwest::Result<HashMap<String, String>> {
    let resp = reqwest::get(url).await?;
    let mut data = HashMap::new();
    if !resp.status().is_success() {
        data.insert("error".to_string(), resp.status().to_string());
        return Ok(data);
    }

    // Ensure the response is valid json

    if !resp.headers().get("content-type").unwrap().to_str().unwrap().contains("application/json") {
        data.insert("error".to_string(), "Response is not valid json".to_string());
        return Ok(data);
    }

    data = resp.json::<HashMap<String, String>>().await?;

    Ok(data)
}


fn load_http_library(lua: &Lua) -> Result<()> {
    lua.context(|lua_ctx| {
        let http_module = lua_ctx.create_table()?;
        let headers = lua_ctx.create_table()?;
        headers.set("User-Agent", "Cumulus/1.0")?;
        headers.set("Accept", "application/json")?;
        http_module.set("headers", headers)?;

        http_module.set("get", lua_ctx.create_function(|ctx, url: String| {
            let response = get_http(&url);
            let response_table = ctx.create_table()?;
            let response_data = response.unwrap();
            for (key, value) in response_data {
                response_table.set(key, value)?;
            }
            Ok(response_table)
        })?)?;

        http_module.set("json", lua_ctx.create_function(|ctx, url: String| {
            let response = get_http_json(&url);
            let response_table = ctx.create_table()?;
            let response_data = response.unwrap();
            for (key, value) in response_data {
                response_table.set(key, value)?;
            }
            Ok(response_table)
        })?)?;

        http_module.set("set_header", lua_ctx.create_function(|ctx, (key, value): (String, String)| {
            let safe_http_module = ctx.globals().get::<_, Table>("http")?;
            let headers = safe_http_module.get::<_, Table>("headers")?;
            headers.set(key, value)?;
            Ok(())
        })?)?;

        lua_ctx.globals().set("http", http_module)?;

        Ok(())
    })?;
    Ok(())
}

fn load_memory_library(lua: &Lua) -> Result<()> {
    lua.context(|lua_ctx| {
        let memory_module = lua_ctx.create_table()?;

        memory_module.set("alloc", lua_ctx.create_function(|_, _: ()| {
            // Allocate 8 bytes of memory by default and return the pointer
            let pointer = Box::into_raw(Box::new([0u8; 8]));
            Ok(pointer as i64)
        })?)?;

        memory_module.set("free", lua_ctx.create_function(|_, pointer: i64| {
            // Free the memory at the pointer
            unsafe {
                let _ = Box::from_raw(pointer as *mut [u8; 8]);
            }
            Ok(())
        })?)?;

        memory_module.set("read", lua_ctx.create_function(|_, pointer: i64| {
            // Read the memory at the pointer
            let mut data = [0u8; 8];
            unsafe {
                data = *Box::from_raw(pointer as *mut [u8; 8]);
            }
            Ok(data)
        })?)?;

        memory_module.set("write", lua_ctx.create_function(|_, (pointer, data): (i64, [u8; 8])| {
            // Write the data to the memory at the pointer
            unsafe {
                *Box::from_raw(pointer as *mut [u8; 8]) = data;
            }
            Ok(())
        })?)?;

        memory_module.set("allocate_int", lua_ctx.create_function(|_, _: ()| {
            // Allocate 8 bytes of memory by default and return the pointer
            let pointer = Box::into_raw(Box::new(0i64));
            Ok(pointer as i64)
        })?)?;

        memory_module.set("read_int", lua_ctx.create_function(|_, pointer: i64| {
            // Read the memory at the pointer
            let mut data = 0i64;
            unsafe {
                data = *Box::from_raw(pointer as *mut i64);
            }
            Ok(data)
        })?)?;

        memory_module.set("write_int", lua_ctx.create_function(|_, (pointer, data): (i64, i64)| {
            // Write the data to the memory at the pointer
            unsafe {
                *Box::from_raw(pointer as *mut i64) = data;
            }
            Ok(())
        })?)?;

        memory_module.set("allocate_float", lua_ctx.create_function(|_, _: ()| {
            // Allocate 8 bytes of memory by default and return the pointer
            let pointer = Box::into_raw(Box::new(0f64));
            Ok(pointer as i64)
        })?)?;

        memory_module.set("read_float", lua_ctx.create_function(|_, pointer: i64| {
            // Read the memory at the pointer
            let mut data = 0f64;
            unsafe {
                data = *Box::from_raw(pointer as *mut f64);
            }
            Ok(data)
        })?)?;

        memory_module.set("write_float", lua_ctx.create_function(|_, (pointer, data): (i64, f64)| {
            // Write the data to the memory at the pointer
            unsafe {
                *Box::from_raw(pointer as *mut f64) = data;
            }
            Ok(())
        })?)?;

        memory_module.set("allocate_string", lua_ctx.create_function(|_, _: ()| {
            // Allocate 8 bytes of memory by default and return the pointer
            let pointer = Box::into_raw(Box::new(String::new()));
            Ok(pointer as i64)
        })?)?;

        memory_module.set("read_string", lua_ctx.create_function(|_, pointer: i64| {
            // Read the memory at the pointer
            let mut data = String::new();
            unsafe {
                data = *Box::from_raw(pointer as *mut String);
            }
            Ok(data)
        })?)?;

        memory_module.set("write_string", lua_ctx.create_function(|_, (pointer, data): (i64, String)| {
            // Write the data to the memory at the pointer
            unsafe {
                *Box::from_raw(pointer as *mut String) = data;
            }
            Ok(())
        })?)?;

        lua_ctx.globals().set("memory", memory_module)?;
        Ok(())
    })?;
    Ok(())
}

fn load_color_library(lua: &Lua) -> Result<()> {
    lua.context(|lua_ctx| {
        let color_module = lua_ctx.create_table()?;

        color_module.set("red", lua_ctx.create_function(|_, args: Variadic<String>| {
            // Color the string red
            let mut colored_string = String::new();
            for arg in args.iter() {
                colored_string.push_str(&arg.red().to_string());
            }
            // Push the colored string to the Lua stack
            Ok(colored_string)
        })?)?;

        color_module.set("green", lua_ctx.create_function(|_, args: Variadic<String>| {
            // Color the string green
            let mut colored_string = String::new();
            for arg in args.iter() {
                colored_string.push_str(&arg.green().to_string());
            }
            // Push the colored string to the Lua stack
            Ok(colored_string)
        })?)?;

        color_module.set("yellow", lua_ctx.create_function(|_, args: Variadic<String>| {
            // Color the string yellow
            let mut colored_string = String::new();
            for arg in args.iter() {
                colored_string.push_str(&arg.yellow().to_string());
            }
            // Push the colored string to the Lua stack
            Ok(colored_string)
        })?)?;

        color_module.set("blue", lua_ctx.create_function(|_, args: Variadic<String>| {
            // Color the string blue
            let mut colored_string = String::new();
            for arg in args.iter() {
                colored_string.push_str(&arg.blue().to_string());
            }
            // Push the colored string to the Lua stack
            Ok(colored_string)
        })?)?;

        color_module.set("magenta", lua_ctx.create_function(|_, args: Variadic<String>| {
            // Color the string magenta
            let mut colored_string = String::new();
            for arg in args.iter() {
                colored_string.push_str(&arg.magenta().to_string());
            }
            // Push the colored string to the Lua stack
            Ok(colored_string)
        })?)?;

        color_module.set("cyan", lua_ctx.create_function(|_, args: Variadic<String>| {
            // Color the string cyan
            let mut colored_string = String::new();
            for arg in args.iter() {
                colored_string.push_str(&arg.cyan().to_string());
            }
            // Push the colored string to the Lua stack
            Ok(colored_string)
        })?)?;

        color_module.set("white", lua_ctx.create_function(|_, args: Variadic<String>| {
            // Color the string white
            let mut colored_string = String::new();
            for arg in args.iter() {
                colored_string.push_str(&arg.white().to_string());
            }
            // Push the colored string to the Lua stack
            Ok(colored_string)
        })?)?;

        color_module.set("black", lua_ctx.create_function(|_, args: Variadic<String>| {
            // Color the string black
            let mut colored_string = String::new();
            for arg in args.iter() {
                colored_string.push_str(&arg.black().to_string());
            }
            // Push the colored string to the Lua stack
            Ok(colored_string)
        })?)?;

        color_module.set("bold", lua_ctx.create_function(|_, args: Variadic<String>| {
            // Bold the string
            let mut bold_string = String::new();
            for arg in args.iter() {
                bold_string.push_str(&arg.bold().to_string());
            }
            // Push the bold string to the Lua stack
            Ok(bold_string)
        })?)?;

        color_module.set("italic", lua_ctx.create_function(|_, args: Variadic<String>| {
            // Italicize the string
            let mut italic_string = String::new();
            for arg in args.iter() {
                italic_string.push_str(&arg.italic().to_string());
            }
            // Push the italic string to the Lua stack
            Ok(italic_string)
        })?)?;

        color_module.set("underline", lua_ctx.create_function(|_, args: Variadic<String>| {
            // Underline the string
            let mut underline_string = String::new();
            for arg in args.iter() {
                underline_string.push_str(&arg.underline().to_string());
            }
            // Push the underline string to the Lua stack
            Ok(underline_string)
        })?)?;

        color_module.set("reverse", lua_ctx.create_function(|_, args: Variadic<String>| {
            // Reverse the string
            let mut reverse_string = String::new();
            for arg in args.iter() {
                reverse_string.push_str(&arg.reverse().to_string());
            }
            // Push the reverse string to the Lua stack
            Ok(reverse_string)
        })?)?;

        lua_ctx.globals().set("color", color_module)?;
        Ok(())
    })?;
    Ok(())
}

fn load_lua_log_library(lua: &Lua) -> Result<()> {
    lua.context(|lua_ctx| {
        let log_lib = lua_ctx.create_table()?;
        log_lib.set(
            "info",
            lua_ctx.create_function(|_, args: Variadic<String>| {
                logger::info(format!("{} {}", "[LUA]".cyan().bold(), args.join(" ")).as_str());
                Ok(())
            })?,
        )?;
        log_lib.set(
            "warn",
            lua_ctx.create_function(|_, args: Variadic<String>| {
                logger::warn(format!("{} {}", "[LUA]".cyan().bold(), args.join(" ")).as_str());
                Ok(())
            })?,
        )?;
        log_lib.set(
            "error",
            lua_ctx.create_function(|_, args: Variadic<String>| {
                logger::error(format!("{} {}", "[LUA]".cyan().bold(), args.join(" ")).as_str());
                Ok(())
            })?,
        )?;
        lua_ctx.globals().set("log", log_lib)?;
        Ok(())
    })
}

fn load_util_library(lua: &Lua) -> Result<()> {
    lua.context(|lua_ctx| {
        let util_lib = lua_ctx.create_table()?;

        lua_ctx.globals().set("util", util_lib)?;
        Ok(())
    })
}

fn lua_interpret_loop(lua: &Lua) -> Result<()> {
    // Create a loop with a prompt
    // Handle interrupt on the loop
    loop {

        // Print the prompt
        print!("> ");
        // Flush the output buffer
        std::io::stdout().flush().unwrap();
        // Read the input
        let mut input = String::new();
        std::io::stdin().read_line(&mut input).unwrap();
        // Remove the newline character
        input = input.trim().to_string();
        // If the input is empty, continue
        if input.is_empty() {
            continue;
        }
        // If the input is "exit", exit
        if input == "exit" {
            lua.context(|lua_ctx| {
                lua_ctx.load("log.info('Exiting Lua interpreter')").exec()?;
                Ok(())
            })?;
            break;
        } else {
            lua_interpret(&lua, &input)?;
        }
    }
    Ok(())
}

fn lua_interpret(lua: &Lua, code: &str) -> Result<()> {
    lua.context(|lua_ctx| {
        let result = lua_ctx.load(code).exec();
        if result.is_err() {
            logger::error(&result.unwrap_err().to_string());
        }
        Ok(())
    })?;
    Ok(())
}
