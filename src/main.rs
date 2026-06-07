use egui_window_glfw_passthrough::{GlfwBackend, GlfwConfig};
use std::sync::Arc;
use global_hotkey::{hotkey::{HotKey, Modifiers, Code}, GlobalHotKeyManager, GlobalHotKeyEvent, HotKeyState};
use std::time::{Duration, Instant};

use windows_sys::Win32::UI::Magnification::{MagInitialize, MagUninitialize};

mod filtros;
mod audio;
mod leitor;
use leitor::GerenciadorLeitor;

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
    
    // INICIALIZA O SISTEMA DE ÁUDIO REATIVO (MOCK)
    let gerenciador_audio = audio::GerenciadorAudio::new();
    let mut limitador_ligado = false;
    let mut limite_volume = 0.5;

    // INICIALIZA O GERENCIADOR DO LEITOR DE TELA (OCR)
    let mut gerenciador_leitor = GerenciadorLeitor::new();

    // ESTADOS DA INTERFACE VISUAL
    let mut filtro_atual = filtros::ModoDaltonismo::Desativado;
    let mut intensidade = 1.0;  
    let mut brilho = 0.0;       
    let mut contraste = 1.0;    
    let mut saturacao = 1.0;    

    // Controle de frame rate para evitar uso de 100% da CPU
    let mut ultimo_frame = Instant::now();

    while !backend.window.should_close() {
        backend.tick();
        let raw_input = backend.take_raw_input();
        
        // 🎯 OTIMIZAÇÃO 1: Passando o contexto correto para a thread do OCR não travar a UI
        gerenciador_leitor.atualizar_tick(&egui_ctx);
        
        // Processa eventos de atalho global
        if let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            // 🎯 OTIMIZAÇÃO 2: Comparação direta via Enum em vez de alocar String com format!()
            if event.id == hotkey.id() && event.state == HotKeyState::Pressed {
                modo_menu_interativo = !modo_menu_interativo;
                if !modo_menu_interativo {
                    gerenciador_leitor.modo_selecao = false;
                }
            }
        }

        unsafe {
            use glow::HasContext;
            gl.clear_color(0.0, 0.0, 0.0, 0.0);
            gl.clear(glow::COLOR_BUFFER_BIT);
        }

        egui_ctx.begin_frame(raw_input);

        // Processa o bloco visual se o menu estiver aberto OU se o leitor estiver selecionando a área
        if modo_menu_interativo || gerenciador_leitor.modo_selecao {
            
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

                        // SEÇÃO DO ON-SCREEN READER (OCR)
                        ui.add_space(10.0);
                        if gerenciador_leitor.exibir_ui(ui, &egui_ctx) {
                            modo_menu_interativo = false; 
                        }

                        // SEÇÃO DE PROTEÇÃO AUDITIVA (MOCK)
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
                            
                            gerenciador_leitor.ativo = false;
                            gerenciador_leitor.regiao_selecionada = None;
                            gerenciador_leitor.texto_extraido = String::from("Nenhum texto processado ainda.");
                        }
                        
                        ui.add_space(4.0);
                        if ui.button("Fechar Software").clicked() {
                            backend.window.set_should_close(true);
                        }
                    });
            } else {
                // 🎯 REQUISITO: Se o menu está oculto mas a seleção de área está rodando...
                gerenciador_leitor.executar_selecao_invisivel(&egui_ctx);
                
                // Assim que o usuário solta o clique do mouse, o `executar_selecao_invisivel` 
                // altera internamente a flag `modo_selecao` para false. Capturamos isso para reabrir a UI!
                if !gerenciador_leitor.modo_selecao {
                    modo_menu_interativo = true;
                }
            }
        }

        let full_output = egui_ctx.end_frame();
        
        // CONTROLE DE INPUT (PASSTHROUGH)
        if gerenciador_leitor.modo_selecao {
            backend.set_passthrough(false);
        } else if modo_menu_interativo {
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

        // 🎯 OTIMIZAÇÃO 3: Limitador de FPS dinâmico para não roubar processamento dos jogos
        let tempo_de_frame = ultimo_frame.elapsed();
        // Alvo de ~144 FPS (aprox. 7ms por frame). Se o processamento foi mais rápido, dorme o resto do tempo.
        if tempo_de_frame < Duration::from_millis(7) {
            std::thread::sleep(Duration::from_millis(7) - tempo_de_frame);
        }
        ultimo_frame = Instant::now();
    }

    // Desliga tudo com segurança ao fechar
    filtros::atualizar_filtros(filtros::ModoDaltonismo::Desativado, 0.0, 1.0, 1.0, 1.0);
    gerenciador_audio.set_ativo(false);
    unsafe { MagUninitialize(); }
    
    egui_glow.destroy();
}