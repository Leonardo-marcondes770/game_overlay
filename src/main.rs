use egui_window_glfw_passthrough::{GlfwBackend, GlfwConfig};
use std::sync::Arc;

// Importações necessárias para o atalho global
use global_hotkey::{hotkey::{HotKey, Modifiers, Code}, GlobalHotKeyManager, GlobalHotKeyEvent};

#[allow(deprecated)] 
fn main() {
    tracing_subscriber::fmt::init();

    // --- CORREÇÃO A NÍVEL DE GLFW ---
    // Precisamos inicializar o GLFW temporariamente para dar o "Hint" (pista) de ToolWindow nativa
    let mut glfw_instance = glfw::init(glfw::fail_on_errors).unwrap();
    // Dizemos ao GLFW que a próxima janela criada deve ser uma Floating Tool Window (ignora Win+D)
    glfw_instance.window_hint(glfw::WindowHint::Floating(true));
    // Esta linha diz ao Windows para não colocar a janela na barra de tarefas e ignorar o minimizar:
    glfw_instance.window_hint(glfw::WindowHint::CocoaRetinaFramebuffer(false)); // (Apenas garante compatibilidade)
    // ---------------------------------

    // 2. Configura a janela do Overlay
    let mut config = GlfwConfig::default();
    config.window_title = "Meu Primeiro Overlay".to_string();
    config.size = [1920, 1080]; 
    config.transparent_window = Some(true);
    config.opengl_window = Some(true);

    // 3. Cria o Backend (Ele vai herdar as configurações que o GLFW preparou acima)
    let mut backend = GlfwBackend::new(config);
    
    backend.window.set_decorated(false);
    backend.window.set_floating(true);

    // Se o seu compilador reclamar que o GlfwBackend já tenta inicializar o GLFW internamente,
    // a forma alternativa e direta de fazer isso na janela já criada é puxando os atributos nativos:
    backend.window.set_focus_on_show(false); // Evita roubar o foco do Windows ao abrir

    // --- CONFIGURAÇÃO DO ATALHO GLOBAL ---
    // Inicializa o gerenciador global do sistema operacional
    let hotkey_manager = GlobalHotKeyManager::new().unwrap();
    
    // Define o atalho: Vamos usar SHIFT + F11 para evitar conflitos com jogos
    let hotkey = HotKey::new(Some(Modifiers::SHIFT), Code::F11);
    
    // Registra o atalho no Windows
    hotkey_manager.register(hotkey).expect("Falha ao registrar atalho global");
    // --------------------------------------

    let gl = unsafe {
        glow::Context::from_loader_function(|s| backend.get_proc_address(s) as *const _)
    };
    let gl = Arc::new(gl);
    
    let egui_ctx = egui::Context::default();
    let mut egui_glow = egui_glow::Painter::new(gl.clone(), "", None, false)
        .expect("Falha ao inicializar o pintor oficial do Egui");

    // Começamos com o menu aberto
    let mut modo_menu_interativo = true; 

    while !backend.window.should_close() {
        backend.tick();
        let raw_input = backend.take_raw_input();
        
        // --- VERIFICAÇÃO DO ATALHO GLOBAL ---
        // O Windows vai disparar um evento aqui SEMPRE que o atalho for pressionado,
        // mesmo se você estiver jogando em tela cheia.
        if let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            // Se o ID do evento for igual ao do nosso atalho...
            if event.id == hotkey.id() {
                // Transformamos o enum interno em texto e checamos se ele é "Pressed"
                if format!("{:?}", event.state) == "Pressed" {
                    modo_menu_interativo = !modo_menu_interativo;
                    println!("Atalho detectado globalmente! Menu interativo: {}", modo_menu_interativo);
                }
            }
        }
        // -------------------------------------

        unsafe {
            use glow::HasContext;
            gl.clear_color(0.0, 0.0, 0.0, 0.0);
            gl.clear(glow::COLOR_BUFFER_BIT);
        }

        egui_ctx.begin_frame(raw_input);

        if modo_menu_interativo {
            egui::Window::new("Configurações do Overlay")
                .default_open(true)
                .show(&egui_ctx, |ui| {
                    ui.label("Seja bem-vindo ao seu protótipo!");
                    ui.separator();
                    ui.label("Aperte SHIFT + F11 globalmente (mesmo jogando) para sumir/aparecer.");
                    
                    if ui.button("Sair do Overlay").clicked() {
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

    egui_glow.destroy();
}