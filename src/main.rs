// src/main.rs
use egui_window_glfw_passthrough::{GlfwBackend, GlfwConfig};
use std::sync::Arc;
use global_hotkey::{hotkey::{HotKey, Modifiers, Code}, GlobalHotKeyManager, GlobalHotKeyEvent};

use windows_sys::Win32::UI::Magnification::{MagInitialize, MagUninitialize};

mod filtros;
mod audio; // IMPORTA O NOVO MÓDULO DE ÁUDIO

#[allow(deprecated)] 
fn main() {
    tracing_subscriber::fmt::init();
    unsafe { MagInitialize(); }

    let mut glfw_instance = glfw::init(glfw::fail_on_errors).unwrap();
    glfw_instance.window_hint(glfw::WindowHint::Floating(true));
    glfw_instance.window_hint(glfw::WindowHint::CocoaRetinaFramebuffer(false));

    let mut config = GlfwConfig::default();
    config.window_title = "Overlay Hardware Pro".to_string();
    config.size = [1920, 1080]; 
    config.transparent_window = Some(true);
    config.opengl_window = Some(true);

    let mut backend = GlfwBackend::new(config);
    backend.window.set_decorated(false);
    backend.window.set_floating(true);
    backend.window.set_focus_on_show(false);

    let hotkey_manager = GlobalHotKeyManager::new().unwrap();
    let hotkey = HotKey::new(Some(Modifiers::SHIFT), Code::F11);
    hotkey_manager.register(hotkey).expect("Falha ao registrar atalho global");

    let gl = unsafe {
        glow::Context::from_loader_function(|s| backend.get_proc_address(s) as *const _)
    };
    let gl = Arc::new(gl);
    
    let egui_ctx = egui::Context::default();
    let mut egui_glow = egui_glow::Painter::new(gl.clone(), "", None, false)
        .expect("Falha ao inicializar o pintor oficial do Egui");

    let mut modo_menu_interativo = true; 
    
    // INICIALIZA O SISTEMA DE ÁUDIO REATIVO
    let gerenciador_audio = audio::GerenciadorAudio::new();
    let mut limitador_ligado = false;
    let mut limite_volume = 0.5; // Corresponde a 50% da barra de pico máximo

    // ESTADOS DA INTERFACE VISUAL
    let mut filtro_atual = filtros::ModoDaltonismo::Desativado;
    let mut intensidade = 1.0;  
    let mut brilho = 0.0;       
    let mut contraste = 1.0;    
    let mut saturacao = 1.0;    

    while !backend.window.should_close() {
        backend.tick();
        let raw_input = backend.take_raw_input();
        
        if let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            if event.id == hotkey.id() && format!("{:?}", event.state) == "Pressed" {
                modo_menu_interativo = !modo_menu_interativo;
            }
        }

        unsafe {
            use glow::HasContext;
            gl.clear_color(0.0, 0.0, 0.0, 0.0);
            gl.clear(glow::COLOR_BUFFER_BIT);
        }

        egui_ctx.begin_frame(raw_input);

        if modo_menu_interativo {
            egui::Window::new("Painel do Overlay Core")
                .default_open(true)
                .show(&egui_ctx, |ui| {
                    ui.heading("Acessibilidade Visual");
                    ui.separator();
                    
                    let mut mudou_video = false;

                    egui::ComboBox::from_label("Modo Daltonismo")
                        .selected_text(match filtro_atual {
                            filtros::ModoDaltonismo::Desativado => "Nenhum (Normal)",
                            filtros::ModoDaltonismo::Protanopia => "Protanopia (Vermelho)",
                            filtros::ModoDaltonismo::Deuteranopia => "Deuteranopia (Verde)",
                            filtros::ModoDaltonismo::Tritanopia => "Tritanopia (Azul)",
                            filtros::ModoDaltonismo::Acromatopsia => "Acromatopsia (Preto e Branco)",
                        })
                        .show_ui(ui, |ui| {
                            mudou_video |= ui.selectable_value(&mut filtro_atual, filtros::ModoDaltonismo::Desativado, "Nenhum (Normal)").changed();
                            mudou_video |= ui.selectable_value(&mut filtro_atual, filtros::ModoDaltonismo::Protanopia, "Protanopia (Vermelho)").changed();
                            mudou_video |= ui.selectable_value(&mut filtro_atual, filtros::ModoDaltonismo::Deuteranopia, "Deuteranopia (Verde)").changed();
                            mudou_video |= ui.selectable_value(&mut filtro_atual, filtros::ModoDaltonismo::Tritanopia, "Tritanopia (Azul)").changed();
                            mudou_video |= ui.selectable_value(&mut filtro_atual, filtros::ModoDaltonismo::Acromatopsia, "Acromatopsia (Preto e Branco)").changed();
                        });

                    if filtro_atual != filtros::ModoDaltonismo::Desativado {
                        ui.add_space(4.0);
                        mudou_video |= ui.add(egui::Slider::new(&mut intensidade, 0.0..=1.0)
                            .text("Intensidade do Filtro")
                            .custom_formatter(|v, _| format!("{:.0}%", v * 100.0))).changed();
                    }

                    ui.add_space(6.0);
                    mudou_video |= ui.add(egui::Slider::new(&mut brilho, -0.5..=0.5).text("Brilho")).changed();
                    mudou_video |= ui.add(egui::Slider::new(&mut contraste, 0.3..=2.0).text("Contraste")).changed();
                    mudou_video |= ui.add(egui::Slider::new(&mut saturacao, 0.0..=3.0).text("Saturação")).changed();

                    if mudou_video {
                        filtros::atualizar_filtros(filtro_atual, brilho, contraste, saturacao, intensidade);
                    }

                    // Nova seção criada especialmente para o controle de Áudio
                    ui.add_space(10.0);
                    ui.heading("Proteção Auditiva Intel");
                    ui.separator();

                    if ui.checkbox(&mut limitador_ligado, "Ativar Limitador de Áudio Reativo").changed() {
                        gerenciador_audio.set_ativo(limitador_ligado);
                    }

                    if limitador_ligado {
                        ui.small("Abaixa o som do PC automaticamente se passar do teto estipulado.");
                        if ui.add(egui::Slider::new(&mut limite_volume, 0.05..=0.95)
                            .text("Teto de Volume")
                            .custom_formatter(|v, _| format!("{:.0}% do Pico", v * 100.0))).changed() 
                        {
                            gerenciador_audio.set_limite(limite_volume);
                        }
                    }

                    ui.separator();
                    if ui.button("Resetar Tudo").clicked() {
                        filtro_atual = filtros::ModoDaltonismo::Desativado;
                        intensidade = 1.0;
                        brilho = 0.0;
                        contraste = 1.0;
                        saturacao = 1.0;
                        filtros::atualizar_filtros(filtro_atual, brilho, contraste, saturacao, intensidade);

                        limitador_ligado = false;
                        gerenciador_audio.set_ativo(false);
                    }
                    
                    ui.add_space(4.0);
                    if ui.button("Fechar Software").clicked() {
                        backend.window.set_should_close(true);
                    }
                });
        }

        let full_output = egui_ctx.end_frame();
        
        if modo_menu_interativo {
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
        egui_glow.paint_and_update_textures(
            [w, h],
            full_output.pixels_per_point,
            &paint_jobs,
            &full_output.textures_delta,
        );

        use glfw::Context;
        backend.window.swap_buffers();
        backend.glfw.poll_events();
    }

    // Desliga tudo com segurança ao fechar
    filtros::atualizar_filtros(filtros::ModoDaltonismo::Desativado, 0.0, 1.0, 1.0, 1.0);
    gerenciador_audio.set_ativo(false);
    unsafe { MagUninitialize(); }
    
    egui_glow.destroy();
}