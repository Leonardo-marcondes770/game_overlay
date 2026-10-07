use egui_window_glfw_passthrough::{GlfwBackend, GlfwConfig};
use std::sync::Arc;
use global_hotkey::{hotkey::{HotKey, Modifiers, Code}, GlobalHotKeyManager, GlobalHotKeyEvent, HotKeyState};

use windows_sys::Win32::UI::Magnification::MagInitialize;
use windows_sys::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN};

mod filtros;
mod audio;
mod leitor;
use leitor::GerenciadorLeitor;

mod lupa;
use lupa::GerenciadorLupa;

mod config;
use config::AppConfig;

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

    // 👇 1. LÊ A TELA VIRTUAL (TODOS OS MONITORES COMBINADOS)
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

    // 👇 2. POSICIONA E ESTICA O OVERLAY SOBRE TODAS AS TELAS
    backend.window.set_pos(v_x, v_y);
    backend.window.set_size(v_w, v_h - 1);

    unsafe {
        let hwnd = windows_sys::Win32::UI::WindowsAndMessaging::FindWindowA(
            std::ptr::null(),
            b"GhostOverlay\0".as_ptr(),
        );
        // O código 0x00000011 é o WDA_EXCLUDEFROMCAPTURE (Esconde da gravação, mas mantém visível pra você)
        if hwnd != 0 {
            windows_sys::Win32::UI::WindowsAndMessaging::SetWindowDisplayAffinity(hwnd, 0x00000011);
        }
    }

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
    let mut config_app = AppConfig::carregar();
    
    // INICIALIZA O SISTEMA DE ÁUDIO REATIVO (MOCK)
    let gerenciador_audio = audio::GerenciadorAudio::new();

    // INICIALIZA O GERENCIADOR DO LEITOR DE TELA (OCR)
    let mut gerenciador_leitor = GerenciadorLeitor::new(egui_ctx.clone());
    gerenciador_leitor.ativo = config_app.leitor_ativo;
    gerenciador_leitor.tts_volume = config_app.tts_volume;
    gerenciador_leitor.tts_velocidade = config_app.tts_velocidade;
    gerenciador_leitor.tts_pitch = config_app.tts_pitch;
    gerenciador_leitor.leitura_periodica = config_app.leitura_periodica;
    gerenciador_leitor.intervalo_leitura = config_app.intervalo_leitura;
    gerenciador_leitor.leitura_mouse = config_app.leitura_mouse;
    gerenciador_leitor.largura_area_mouse = config_app.largura_area_mouse;
    gerenciador_leitor.altura_area_mouse = config_app.altura_area_mouse;
    gerenciador_leitor.atualizar_config_tts();

    let mut gerenciador_lupa = GerenciadorLupa::new(egui_ctx.clone());
    gerenciador_lupa.ativo = config_app.lupa_ativa;
    gerenciador_lupa.modo_mouse = config_app.lupa_modo_mouse;
    gerenciador_lupa.fator_zoom = config_app.lupa_fator_zoom;
    gerenciador_lupa.tamanho_janela = config_app.lupa_tamanho_janela;
    gerenciador_lupa.pixel_art = config_app.lupa_pixel_art;

    filtros::atualizar_filtros(config_app.filtro_atual, config_app.brilho, config_app.contraste, config_app.saturacao, config_app.intensidade);

    while !backend.window.should_close() {
        backend.tick();
        let raw_input = backend.take_raw_input();
        
        gerenciador_leitor.atualizar_tick(&egui_ctx);
        
        // Processa eventos de atalho global (Shift + F11)
        if let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
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
                egui::Window::new("Ghost Overlay")
                    .default_open(true)
                    .show(&egui_ctx, |ui| {
                        ui.heading("Overlay de Acessibilidade");
                        ui.separator();
                        
                        let mut mudou_video = false;

                        egui::ComboBox::from_label("Filtro para Daltonismo")
                            .selected_text(match config_app.filtro_atual {
                                filtros::ModoDaltonismo::Desativado => "Nenhum (Normal)",
                                filtros::ModoDaltonismo::Protanopia => "Protanopia (Vermelho)",
                                filtros::ModoDaltonismo::Deuteranopia => "Deuteranopia (Verde)",
                                filtros::ModoDaltonismo::Tritanopia => "Tritanopia (Azul)",
                                filtros::ModoDaltonismo::Acromatopsia => "Acromatopsia (Preto e Branco)",
                            })
                            .show_ui(ui, |ui| {
                                mudou_video |= ui.selectable_value(&mut config_app.filtro_atual, filtros::ModoDaltonismo::Desativado, "Nenhum (Normal)").changed();
                                mudou_video |= ui.selectable_value(&mut config_app.filtro_atual, filtros::ModoDaltonismo::Protanopia, "Protanopia (Vermelho)").changed();
                                mudou_video |= ui.selectable_value(&mut config_app.filtro_atual, filtros::ModoDaltonismo::Deuteranopia, "Deuteranopia (Verde)").changed();
                                mudou_video |= ui.selectable_value(&mut config_app.filtro_atual, filtros::ModoDaltonismo::Tritanopia, "Tritanopia (Azul)").changed();
                                mudou_video |= ui.selectable_value(&mut config_app.filtro_atual, filtros::ModoDaltonismo::Acromatopsia, "Acromatopsia (Preto e Branco)").changed();
                            });

                        if config_app.filtro_atual != filtros::ModoDaltonismo::Desativado {
                            ui.add_space(4.0);
                            mudou_video |= ui.add(egui::Slider::new(&mut config_app.intensidade, 0.0..=1.0)
                                .text("Intensidade do Filtro")
                                .custom_formatter(|v, _| format!("{:.0}%", v * 100.0))).changed();
                        }

                        ui.add_space(6.0);
                        mudou_video |= ui.add(egui::Slider::new(&mut config_app.brilho, -0.5..=0.5).text("Brilho")).changed();
                        mudou_video |= ui.add(egui::Slider::new(&mut config_app.contraste, 0.3..=2.0).text("Contraste")).changed();
                        mudou_video |= ui.add(egui::Slider::new(&mut config_app.saturacao, 0.0..=3.0).text("Saturação")).changed();

                        if mudou_video {
                            filtros::atualizar_filtros(config_app.filtro_atual, config_app.brilho, config_app.contraste, config_app.saturacao, config_app.intensidade);
                        }

                        // SEÇÃO DO ON-SCREEN READER (OCR)
                        ui.add_space(10.0);
                        if gerenciador_leitor.exibir_ui(ui, &egui_ctx) {
                            modo_menu_interativo = false; 
                        }

                        ui.add_space(10.0);
                        if gerenciador_lupa.exibir_ui(ui) {
                            modo_menu_interativo = false; // Esconde o menu se for selecionar alvo
                        }

                        // SEÇÃO DE PROTEÇÃO AUDITIVA
                        ui.add_space(10.0);
                        ui.heading("Proteção Auditiva");
                        ui.separator();

                        if ui.checkbox(&mut config_app.limitador_ligado, "Ativar Limitador de Áudio Reativo").changed() {
                            gerenciador_audio.set_ativo(config_app.limitador_ligado);
                        }

                        if config_app.limitador_ligado {
                            ui.small("Abaixa o som do PC automaticamente se passar do teto estipulado.");
                            if ui.add(egui::Slider::new(&mut config_app.limite_volume, 0.05..=0.95)
                                .text("Teto de Volume")
                                .custom_formatter(|v, _| format!("{:.0}% do Pico", v * 100.0))).changed() 
                            {
                                gerenciador_audio.set_limite(config_app.limite_volume);
                            }
                        }

                        ui.separator();
                        if ui.button("Resetar Tudo").clicked() {
                            // Volta a configuração inteira para o padrão
                            config_app = AppConfig::default(); 
                            
                            filtros::atualizar_filtros(config_app.filtro_atual, config_app.brilho, config_app.contraste, config_app.saturacao, config_app.intensidade);

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
                // Se o menu está oculto mas a seleção de área está rodando...
                gerenciador_leitor.executar_selecao_invisivel(&egui_ctx);
                gerenciador_lupa.executar_selecao_centro(&egui_ctx);
                
                // Assim que o usuário solta o clique do mouse, o `executar_selecao_invisivel` 
                // altera internamente a flag `modo_selecao` para false. Capturamos isso para reabrir a UI!
                if !gerenciador_leitor.modo_selecao && !gerenciador_lupa.selecionando_centro {
                    modo_menu_interativo = true;
                }
            }
        }

        // Renderiza o debug sempre, independente de o menu estar aberto ou não
        gerenciador_leitor.renderizar_overlay_debug(&egui_ctx);
        gerenciador_lupa.atualizar_tick(&egui_ctx);
        
        // 👇 CORREÇÃO 1: Faltava renderizar a janela da lupa na tela!
        gerenciador_lupa.renderizar_janela(&egui_ctx);

        let full_output = egui_ctx.end_frame();
        
        // 👇 CORREÇÃO 2: Controle de Input (Passthrough) consertado para a lupa!
        if gerenciador_leitor.modo_selecao || gerenciador_lupa.selecionando_centro {
            // Se estamos definindo uma área (seja leitor ou lupa), captura o mouse!
            backend.set_passthrough(false);
        } else if modo_menu_interativo {
            // Se o menu principal está aberto
            if egui_ctx.wants_pointer_input() || egui_ctx.is_using_pointer() {
                backend.set_passthrough(false);
            } else {
                backend.set_passthrough(true);
            }
        } else {
            // Se estamos jogando e não definindo alvos, o mouse atravessa para o jogo
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
        backend.glfw.wait_events_timeout(0.016);
    }

    // --- FIM DO LAÇO WHILE ---

    // 1. Atualizamos nossa struct com o estado final do leitor antes de salvar
    config_app.leitor_ativo = gerenciador_leitor.ativo;
    config_app.tts_volume = gerenciador_leitor.tts_volume;
    config_app.tts_velocidade = gerenciador_leitor.tts_velocidade;
    config_app.tts_pitch = gerenciador_leitor.tts_pitch;
    config_app.leitura_periodica = gerenciador_leitor.leitura_periodica;
    config_app.intervalo_leitura = gerenciador_leitor.intervalo_leitura;
    config_app.leitura_mouse = gerenciador_leitor.leitura_mouse;
    config_app.largura_area_mouse = gerenciador_leitor.largura_area_mouse;
    config_app.altura_area_mouse = gerenciador_leitor.altura_area_mouse;
    config_app.lupa_ativa = gerenciador_lupa.ativo;
    config_app.lupa_modo_mouse = gerenciador_lupa.modo_mouse;
    config_app.lupa_fator_zoom = gerenciador_lupa.fator_zoom;
    config_app.lupa_tamanho_janela = gerenciador_lupa.tamanho_janela;
    config_app.lupa_pixel_art = gerenciador_lupa.pixel_art;
    
    // 2. Salva as configurações em %APPDATA%/GhostOverlay/config.json
    config_app.salvar();

    // 3. Desliga tudo com segurança ao fechar
    filtros::atualizar_filtros(filtros::ModoDaltonismo::Desativado, 0.0, 1.0, 1.0, 1.0);
    gerenciador_audio.set_ativo(false);
    unsafe { windows_sys::Win32::UI::Magnification::MagUninitialize(); }
    
    egui_glow.destroy();
    }