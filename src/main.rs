// src/main.rs
use egui_window_glfw_passthrough::{GlfwBackend, GlfwConfig};
use std::sync::Arc;
use global_hotkey::{hotkey::{HotKey, Modifiers, Code}, GlobalHotKeyManager, GlobalHotKeyEvent, HotKeyState};

use windows_sys::Win32::UI::Magnification::MagInitialize;
use windows_sys::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN};

mod filtros;
mod audio;
mod leitor;
mod lupa;
mod config;
mod tradutor;
mod app;

use app::GhostOverlayApp;

fn configurar_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        println!("CRASH FATAL DETECTADO: {}", info);
        let identidade = windows_sys::Win32::UI::Magnification::MAGCOLOREFFECT {
            transform: [
                1.0, 0.0, 0.0, 0.0, 0.0,
                0.0, 1.0, 0.0, 0.0, 0.0,
                0.0, 0.0, 1.0, 0.0, 0.0,
                0.0, 0.0, 0.0, 1.0, 0.0,
                0.0, 0.0, 0.0, 0.0, 1.0,
            ],
        };
        unsafe {
            windows_sys::Win32::UI::Magnification::MagSetFullscreenColorEffect(&identidade);
            windows_sys::Win32::UI::Magnification::MagUninitialize();
        }
    }));
}

#[allow(deprecated)] 
fn main() {
    configurar_panic_hook();
    tracing_subscriber::fmt::init();
    unsafe { MagInitialize(); }

    let mut glfw_instance = glfw::init(glfw::fail_on_errors).unwrap();
    glfw_instance.window_hint(glfw::WindowHint::Floating(true));
    glfw_instance.window_hint(glfw::WindowHint::CocoaRetinaFramebuffer(false));

    let v_x = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
    let v_y = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
    let v_w = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
    let v_h = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };

    let mut config = GlfwConfig::default();
    config.window_title = "GhostOverlay".to_string();
    config.size = [v_w as u32, (v_h - 1) as u32]; 
    config.transparent_window = Some(true);
    config.opengl_window = Some(true);

    let mut backend = GlfwBackend::new(config);
    backend.window.set_decorated(false);
    backend.window.set_floating(true);
    backend.window.set_focus_on_show(false);

    backend.window.set_pos(v_x, v_y);
    backend.window.set_size(v_w, v_h - 1);

    unsafe {
        let hwnd = windows_sys::Win32::UI::WindowsAndMessaging::FindWindowA(std::ptr::null(), b"GhostOverlay\0".as_ptr());
        if hwnd != 0 {
            windows_sys::Win32::UI::WindowsAndMessaging::SetWindowDisplayAffinity(hwnd, 0x00000011);
        }
    }

    let hotkey_manager = GlobalHotKeyManager::new().unwrap();
    let hotkey = HotKey::new(Some(Modifiers::SHIFT), Code::F11);
    hotkey_manager.register(hotkey).expect("Falha ao registrar atalho global");

    let gl = unsafe { glow::Context::from_loader_function(|s| backend.get_proc_address(s) as *const _) };
    let gl = Arc::new(gl);
    
    let egui_ctx = egui::Context::default();
    let mut egui_glow = egui_glow::Painter::new(gl.clone(), "", None, false).expect("Falha ao inicializar o Egui");

    // 👇 INICIALIZAÇÃO DO NOVO APP CENTRALIZADO!
    let mut app = GhostOverlayApp::new(&egui_ctx);

    while !backend.window.should_close() {
        backend.tick();
        let raw_input = backend.take_raw_input();
        
        // Atualiza os ticks em background (Lupa e Leitor)
        app.atualizar_tick(&egui_ctx);
        
        if let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            if event.id == hotkey.id() && event.state == HotKeyState::Pressed {
                app.modo_menu_interativo = !app.modo_menu_interativo;
                if !app.modo_menu_interativo {
                    app.leitor.modo_selecao = false;
                    app.lupa.selecionando_centro = false;
                }
            }
        }

        unsafe {
            use glow::HasContext;
            gl.clear_color(0.0, 0.0, 0.0, 0.0);
            gl.clear(glow::COLOR_BUFFER_BIT);
        }

        egui_ctx.begin_frame(raw_input);

        // 👇 RENDERIZA TODA A UI DE FORMA LIMPA!
        app.renderizar_ui(&egui_ctx);

        if app.deve_fechar {
            backend.window.set_should_close(true);
        }

        let full_output = egui_ctx.end_frame();
        
        // Controle de Input (Passthrough)
        if app.leitor.modo_selecao || app.lupa.selecionando_centro {
            backend.set_passthrough(false);
        } else if app.modo_menu_interativo {
            if egui_ctx.wants_pointer_input() || egui_ctx.is_using_pointer() {
                backend.set_passthrough(false);
            } else {
                backend.set_passthrough(true);
            }
        } else {
            backend.set_passthrough(true);
        }

        let paint_jobs = egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);

        let [w, h] = backend.framebuffer_size_physical;
        egui_glow.paint_and_update_textures([w, h], full_output.pixels_per_point, &paint_jobs, &full_output.textures_delta);

        use glfw::Context;
        backend.window.swap_buffers();
        backend.glfw.wait_events_timeout(0.016);
    }

    // Fim da execução - Delega salvamento e encerramento ao App
    app.salvar_e_fechar();

    unsafe { windows_sys::Win32::UI::Magnification::MagUninitialize(); }
    egui_glow.destroy();
}