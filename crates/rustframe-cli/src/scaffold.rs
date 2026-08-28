use std::{
    env, fs,
    io::{self, IsTerminal, Write},
    path::Path,
    process::Command,
};

use crate::command::{NewArgs, PackageManager, Template};

const ICON: &str = include_str!("../templates/frontend/assets/icon.svg");
const SCHEMA: &str = include_str!("../templates/data/schema.json");

pub fn create(args: &NewArgs) -> Result<(), String> {
    validate_name(&args.name)?;
    let template = select_template(args.template)?;
    let package_manager = select_package_manager(args.package_manager)?;
    let root = env::current_dir()
        .map_err(|error| format!("failed to resolve current directory: {error}"))?
        .join(&args.name);
    if root.exists() {
        return Err(format!(
            "project directory already exists: {}",
            root.display()
        ));
    }

    for directory in ["src", "data/seeds", "data/migrations", "public", "assets"] {
        fs::create_dir_all(root.join(directory)).map_err(|error| {
            format!(
                "failed to create '{}': {error}",
                root.join(directory).display()
            )
        })?;
    }

    let title = humanize(&args.name);
    write(
        &root.join("rustframe.json"),
        &manifest(&args.name, &title, template, package_manager),
    )?;
    write(
        &root.join("package.json"),
        &package_json(&args.name, template),
    )?;
    write(&root.join("index.html"), &index_html(&title, template))?;
    write(&root.join("data/schema.json"), SCHEMA)?;
    write(
        &root.join("assets/icon.svg"),
        &ICON
            .replace("{{app_title}}", &title)
            .replace("{{app_monogram}}", &monogram(&title)),
    )?;
    write(
        &root.join(".gitignore"),
        "node_modules/\ndist/\ntarget/\n*.log\n",
    )?;
    write(
        &root.join("README.md"),
        &readme(&args.name, package_manager),
    )?;

    match template {
        Template::VanillaTs => write(&root.join("src/main.ts"), VANILLA_TS)?,
        Template::VanillaJs => write(&root.join("src/main.js"), VANILLA_JS)?,
        Template::ReactTs => {
            write(&root.join("src/main.tsx"), REACT_MAIN)?;
            write(&root.join("src/App.tsx"), REACT_APP)?;
            write(&root.join("vite.config.ts"), REACT_VITE)?;
        }
        Template::VueTs => {
            write(&root.join("src/main.ts"), VUE_MAIN)?;
            write(&root.join("src/App.vue"), VUE_APP)?;
            write(&root.join("vite.config.ts"), VUE_VITE)?;
        }
        Template::SvelteTs => {
            write(&root.join("src/main.ts"), SVELTE_MAIN)?;
            write(&root.join("src/App.svelte"), SVELTE_APP)?;
            write(&root.join("vite.config.ts"), SVELTE_VITE)?;
        }
    }
    write(&root.join("src/style.css"), STYLE)?;

    crate::codegen::generate(&root, false)?;
    if args.install {
        install(&root, package_manager)?;
    }

    println!("Created RustFrame project at {}", root.display());
    println!("  template: {}", template.as_str());
    println!("  package manager: {}", package_manager.as_str());
    println!();
    println!("Next:");
    println!("  cd {}", args.name);
    if !args.install {
        println!("  {} install", package_manager.as_str());
    }
    println!("  rustframe dev");
    Ok(())
}

fn select_template(selected: Option<Template>) -> Result<Template, String> {
    if let Some(selected) = selected {
        return Ok(selected);
    }
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Ok(Template::default());
    }
    println!("Choose a frontend template:");
    println!("  1. vanilla-ts (recommended)");
    println!("  2. vanilla-js");
    println!("  3. react-ts");
    println!("  4. vue-ts");
    println!("  5. svelte-ts");
    match prompt_choice("Template", 1)? {
        1 => Ok(Template::VanillaTs),
        2 => Ok(Template::VanillaJs),
        3 => Ok(Template::ReactTs),
        4 => Ok(Template::VueTs),
        5 => Ok(Template::SvelteTs),
        _ => Err("template choice must be between 1 and 5".into()),
    }
}

fn select_package_manager(selected: Option<PackageManager>) -> Result<PackageManager, String> {
    if let Some(selected) = selected {
        return Ok(selected);
    }
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Ok(PackageManager::default());
    }
    println!("Choose a package manager:");
    println!("  1. npm (recommended)");
    println!("  2. pnpm");
    println!("  3. yarn");
    println!("  4. bun");
    match prompt_choice("Package manager", 1)? {
        1 => Ok(PackageManager::Npm),
        2 => Ok(PackageManager::Pnpm),
        3 => Ok(PackageManager::Yarn),
        4 => Ok(PackageManager::Bun),
        _ => Err("package manager choice must be between 1 and 4".into()),
    }
}

fn prompt_choice(label: &str, default: usize) -> Result<usize, String> {
    print!("{label} [{default}]: ");
    io::stdout()
        .flush()
        .map_err(|error| format!("failed to write prompt: {error}"))?;
    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|error| format!("failed to read prompt: {error}"))?;
    let input = input.trim();
    if input.is_empty() {
        return Ok(default);
    }
    input
        .parse()
        .map_err(|_| format!("{label} choice must be a number"))
}

fn manifest(name: &str, title: &str, template: Template, manager: PackageManager) -> String {
    let source = if template == Template::ReactTs {
        "/src/main.tsx"
    } else if template == Template::VanillaJs {
        "/src/main.js"
    } else {
        "/src/main.ts"
    };
    let generated = if template.uses_typescript() {
        "src/rustframe.generated.ts"
    } else {
        "src/rustframe.generated.js"
    };
    let run = match manager {
        PackageManager::Npm => "npm run",
        PackageManager::Pnpm => "pnpm",
        PackageManager::Yarn => "yarn",
        PackageManager::Bun => "bun run",
    };
    format!(r#"{{
  "$schema": "https://othmaneblial.github.io/rustframe/schemas/v1/rustframe.schema.json",
  "schemaVersion": 1,
  "app": {{
    "id": "{name}",
    "title": "{title}",
    "windows": [{{ "id": "main", "title": "{title}", "route": "/", "width": 1280, "height": 820 }}]
  }},
  "frontend": {{
    "devCommand": "{run} dev -- --host 127.0.0.1",
    "buildCommand": "{run} build",
    "devUrl": "http://127.0.0.1:5173",
    "distDir": "dist",
    "generatedTypes": "{generated}"
  }},
  "security": {{
    "model": "local-first",
    "csp": "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self' http://127.0.0.1:* ws://127.0.0.1:*; object-src 'none'; base-uri 'none'; frame-ancestors 'none'",
    "permissions": [{{
      "window": "main",
      "allow": ["db:read", "db:write", "db:backup", "db:restore", "fs:grants:read", "fs:grants:write", "fs:grants:watch", "dialog:open", "dialog:save", "window:create"]
    }}]
  }},
  "database": {{ "schema": "data/schema.json", "seeds": "data/seeds", "migrations": "data/migrations" }},
  "filesystem": {{ "roots": [], "persistGrants": true }},
  "shell": {{ "commands": [] }},
  "packaging": {{
    "version": "0.1.0",
    "identifier": "dev.rustframe.{name}",
    "description": "{title}",
    "icon": "assets/icon.svg",
    "linux": {{ "categories": ["Utility"], "keywords": ["local-first", "rustframe"] }},
    "windows": {{}},
    "macos": {{}}
  }},
  "_source": "{source}"
}}
"#).replace(",\n  \"_source\": \"/src/main.ts\"", "").replace(",\n  \"_source\": \"/src/main.tsx\"", "").replace(",\n  \"_source\": \"/src/main.js\"", "")
}

fn package_json(name: &str, template: Template) -> String {
    let mut dependencies = serde_json::Map::new();
    dependencies.insert(
        "rustframe-api".into(),
        serde_json::json!(format!("={}", env!("CARGO_PKG_VERSION"))),
    );
    match template {
        Template::ReactTs => {
            dependencies.insert("@vitejs/plugin-react".into(), serde_json::json!("^5.0.0"));
            dependencies.insert("react".into(), serde_json::json!("^19.1.0"));
            dependencies.insert("react-dom".into(), serde_json::json!("^19.1.0"));
            dependencies.insert("@types/react".into(), serde_json::json!("^19.1.0"));
            dependencies.insert("@types/react-dom".into(), serde_json::json!("^19.1.0"));
        }
        Template::VueTs => {
            dependencies.insert("@vitejs/plugin-vue".into(), serde_json::json!("^6.0.0"));
            dependencies.insert("vue".into(), serde_json::json!("^3.5.0"));
        }
        Template::SvelteTs => {
            dependencies.insert(
                "@sveltejs/vite-plugin-svelte".into(),
                serde_json::json!("^6.0.0"),
            );
            dependencies.insert("svelte".into(), serde_json::json!("^5.0.0"));
        }
        _ => {}
    }
    if template.uses_typescript() {
        dependencies.insert("typescript".into(), serde_json::json!("^5.9.0"));
    }
    dependencies.insert("vite".into(), serde_json::json!("^7.0.0"));
    let value = serde_json::json!({
        "name": name,
        "private": true,
        "version": "0.1.0",
        "type": "module",
        "scripts": { "dev": "vite", "build": "vite build", "typecheck": "tsc --noEmit" },
        "dependencies": dependencies,
    });
    serde_json::to_string_pretty(&value).expect("package JSON") + "\n"
}

fn index_html(title: &str, template: Template) -> String {
    let source = if template == Template::ReactTs {
        "/src/main.tsx"
    } else if template == Template::VanillaJs {
        "/src/main.js"
    } else {
        "/src/main.ts"
    };
    format!(
        r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <meta http-equiv="Content-Security-Policy" content="default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; connect-src 'self' http://127.0.0.1:* ws://127.0.0.1:*; object-src 'none'; base-uri 'none'; frame-ancestors 'none'" />
    <title>{title}</title>
  </head>
  <body>
    <main id="app"></main>
    <script type="module" src="{source}"></script>
  </body>
</html>
"#
    )
}

fn readme(name: &str, manager: PackageManager) -> String {
    format!(
        "# {name}\n\n```bash\n{} install\nrustframe dev\nrustframe validate\nrustframe build\nrustframe package\n```\n",
        manager.as_str()
    )
}

fn install(root: &Path, manager: PackageManager) -> Result<(), String> {
    let status = Command::new(manager.as_str())
        .arg("install")
        .current_dir(root)
        .status()
        .map_err(|error| format!("failed to start {} install: {error}", manager.as_str()))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{} install failed with {status}", manager.as_str()))
    }
}

fn write(path: &Path, contents: &str) -> Result<(), String> {
    fs::write(path, contents)
        .map_err(|error| format!("failed to write '{}': {error}", path.display()))
}

fn validate_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || !name.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
    {
        return Err("project name must contain only lowercase letters, digits, and hyphens".into());
    }
    Ok(())
}

fn humanize(name: &str) -> String {
    name.split('-')
        .map(|part| {
            let mut chars = part.chars();
            chars
                .next()
                .map(|first| first.to_ascii_uppercase().to_string() + chars.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn monogram(title: &str) -> String {
    title
        .split_whitespace()
        .filter_map(|part| part.chars().next())
        .take(2)
        .collect::<String>()
        .to_uppercase()
}

const STYLE: &str = r#":root { font-family: Inter, ui-sans-serif, system-ui, sans-serif; color: #edf4ff; background: #08111f; }
* { box-sizing: border-box; }
body { margin: 0; min-width: 320px; min-height: 100vh; background: radial-gradient(circle at top left, #17345d, #08111f 55%); }
.shell { width: min(760px, calc(100% - 40px)); margin: 12vh auto; padding: 48px; border: 1px solid #29466d; border-radius: 24px; background: #0d1a2dcc; box-shadow: 0 24px 80px #0008; }
h1 { margin: 0 0 12px; font-size: clamp(2.5rem, 7vw, 5rem); letter-spacing: -0.06em; }
p { color: #abc0dc; line-height: 1.7; }
button { border: 0; border-radius: 999px; padding: 12px 20px; color: #07121f; background: #70e1c1; font: inherit; font-weight: 700; cursor: pointer; }
"#;

const VANILLA_TS: &str = r##"import { getRustFrame } from "rustframe-api";
import type { AppRustFrameClient } from "./rustframe.generated";
import "./style.css";

document.querySelector<HTMLDivElement>("#app")!.innerHTML = `<section class="shell"><p>RustFrame / local-first</p><h1>Your tool starts here.</h1><p>Typed SQLite and capability-scoped desktop APIs are ready.</p><button id="inspect">Inspect runtime</button><pre id="output"></pre></section>`;
document.querySelector("#inspect")?.addEventListener("click", async () => {
  const rustframe = getRustFrame() as AppRustFrameClient;
  document.querySelector("#output")!.textContent = JSON.stringify(await rustframe.db.info(), null, 2);
});
"##;

const VANILLA_JS: &str = r##"import { getRustFrame } from "rustframe-api";
import "./rustframe.generated.js";
import "./style.css";

document.querySelector("#app").innerHTML = `<section class="shell"><p>RustFrame / local-first</p><h1>Your tool starts here.</h1><p>The plain JavaScript bridge is ready.</p><button id="inspect">Inspect runtime</button><pre id="output"></pre></section>`;
document.querySelector("#inspect")?.addEventListener("click", async () => {
  document.querySelector("#output").textContent = JSON.stringify(await getRustFrame().db.info(), null, 2);
});
"##;

const REACT_MAIN: &str = "import React from 'react';\nimport { createRoot } from 'react-dom/client';\nimport App from './App';\nimport './style.css';\ncreateRoot(document.getElementById('app')!).render(<React.StrictMode><App /></React.StrictMode>);\n";
const REACT_APP: &str = "export default function App() { return <section className=\"shell\"><p>RustFrame / React</p><h1>Your tool starts here.</h1><p>Typed local-first desktop APIs are ready.</p></section>; }\n";
const REACT_VITE: &str = "import { defineConfig } from 'vite';\nimport react from '@vitejs/plugin-react';\nexport default defineConfig({ plugins: [react()] });\n";
const VUE_MAIN: &str = "import { createApp } from 'vue';\nimport App from './App.vue';\nimport './style.css';\ncreateApp(App).mount('#app');\n";
const VUE_APP: &str = "<template><section class=\"shell\"><p>RustFrame / Vue</p><h1>Your tool starts here.</h1><p>Typed local-first desktop APIs are ready.</p></section></template>\n";
const VUE_VITE: &str = "import { defineConfig } from 'vite';\nimport vue from '@vitejs/plugin-vue';\nexport default defineConfig({ plugins: [vue()] });\n";
const SVELTE_MAIN: &str = "import App from './App.svelte';\nimport './style.css';\nnew App({ target: document.getElementById('app')! });\n";
const SVELTE_APP: &str = "<section class=\"shell\"><p>RustFrame / Svelte</p><h1>Your tool starts here.</h1><p>Typed local-first desktop APIs are ready.</p></section>\n";
const SVELTE_VITE: &str = "import { defineConfig } from 'vite';\nimport { svelte } from '@sveltejs/vite-plugin-svelte';\nexport default defineConfig({ plugins: [svelte()] });\n";
