use eframe::egui;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([880.0, 580.0])
            .with_decorations(false) 
            .with_transparent(true),
        ..Default::default()
    };

    eframe::run_native(
        "GhostOverlay Sandbox",
        options,
        Box::new(|_cc| Ok(Box::new(SandboxApp::default()))),
    )
}

fn aplicar_tema_ghost(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    let mut visuals = egui::Visuals::dark();
    
    let cor_destaque_ciano = egui::Color32::from_rgb(0, 197, 175); 
    let cor_texto_padrao = egui::Color32::from_rgb(240, 241, 241);  
    visuals.widgets.noninteractive.bg_stroke = egui::Stroke::NONE;

    // 1. Estado Parado (Inactive)
    let cor_botoes = egui::Color32::from_rgb(34, 47, 62); 
    visuals.widgets.inactive.bg_fill = cor_botoes;
    visuals.widgets.inactive.weak_bg_fill = cor_botoes;
    visuals.widgets.inactive.bg_stroke = egui::Stroke::NONE;
    visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(255, 255, 255));

    // 2. Estado Highlight (Hovered)
    let cor_hover = egui::Color32::from_rgb(15, 107, 209); 
    visuals.widgets.hovered.bg_fill = cor_hover;
    visuals.widgets.hovered.weak_bg_fill = cor_hover; 
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(255, 255, 255));
    
    // 3. Estado Clicando (Active)
    visuals.widgets.active.bg_fill = cor_destaque_ciano;
    visuals.widgets.active.weak_bg_fill = cor_destaque_ciano;
    visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(255, 255, 255));
    
    visuals.window_fill = egui::Color32::TRANSPARENT; 
    visuals.window_stroke = egui::Stroke::NONE; 
    visuals.selection.bg_fill = cor_destaque_ciano; 
    
    // 👇 A MÁGICA ACONTECE AQUI 👇
    visuals.override_text_color = None; // Desliga a trava global de cor
    visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, cor_texto_padrao); // Mantém os títulos brancos
    
    let raio_borda = 4.0; 
    visuals.window_rounding = egui::Rounding::same(raio_borda);
    visuals.widgets.noninteractive.rounding = egui::Rounding::same(raio_borda);
    visuals.widgets.inactive.rounding = egui::Rounding::same(raio_borda);
    visuals.widgets.hovered.rounding = egui::Rounding::same(raio_borda);
    visuals.widgets.active.rounding = egui::Rounding::same(raio_borda);

    style.spacing.item_spacing = egui::Vec2::new(12.0, 14.0); 
    style.spacing.slider_width = 150.0; 
    style.spacing.interact_size.y = 22.0; 
    style.spacing.button_padding = egui::Vec2::new(16.0, 8.0);

    use egui::{FontId, TextStyle};
    style.text_styles = [
        (TextStyle::Heading, FontId::proportional(18.0)), 
        (TextStyle::Body, FontId::proportional(14.0)),    
        (TextStyle::Button, FontId::proportional(14.0)),  
    ].into();

    style.visuals = visuals;
    ctx.set_style(style);
}

struct SandboxApp {
    contraste: f32,
    saturacao: f32,
    brilho: f32,
    leitor_ligado: bool,
    filtro_selecionado: usize,
    aba_atual: usize,
}

impl Default for SandboxApp {
    fn default() -> Self {
        Self { contraste: 45.0, saturacao: 75.0, brilho: 60.0, leitor_ligado: true, filtro_selecionado: 0, aba_atual: 0 }
    }
}

impl eframe::App for SandboxApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        aplicar_tema_ghost(ctx);

        let frame_janela = egui::Frame::none()
            .fill(egui::Color32::from_rgb(34, 47, 62))
            .rounding(8.0) 
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(34, 47, 62)))
            .inner_margin(0.0);

        let frame_sidebar = egui::Frame::none()
            .fill(egui::Color32::from_rgb(11, 18, 26))
            .inner_margin(egui::Margin::same(20.0))
            .rounding(egui::Rounding { nw: 8.0, sw: 8.0, ne: 0.0, se: 0.0 });

        let frame_cards = egui::Frame::none()
            .fill(egui::Color32::from_rgb(50, 78, 104)) 
            .rounding(6.0)
            .inner_margin(egui::Margin::same(16.0));

        egui::CentralPanel::default().frame(frame_janela).show(ctx, |ui| {
            // A vez de usar colunas, usamos painéis laterais (SidePanel) que não crasham com a matemática.
            
            // ==========================================
            // 1. BARRA LATERAL (SIDEBAR ESQUERDA)
            // ==========================================
            egui::SidePanel::left("sidebar_esquerda")
                .frame(frame_sidebar)
                .resizable(false)
                .exact_width(170.0) // Fixamos a largura com segurança
                .show_inside(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(20.0);
                        ui.label(egui::RichText::new("👻").size(60.0));
                        ui.heading(egui::RichText::new("Perfis de jogos").color(egui::Color32::from_rgb(0, 197, 175)).strong());
                    });

                    ui.add_space(30.0);
                    ui.selectable_value(&mut self.aba_atual, 0, "  Perfil 1");
                    ui.selectable_value(&mut self.aba_atual, 1, "  Perfil 2");
                    ui.selectable_value(&mut self.aba_atual, 2, "  Perfil 3");
                    ui.selectable_value(&mut self.aba_atual, 3, "  Perfil 4");

                    ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                        ui.add_space(20.0);
                        ui.selectable_value(&mut self.aba_atual, 5, "⚙ Configurações");
                        ui.selectable_value(&mut self.aba_atual, 4, "📖 Guia");
                    });
                });

            // ==========================================
            // 2. COLUNA DE CONTEÚDO (DIREITA EXTREMA)
            // ==========================================
            // Desenhamos a coluna da direita primeiro usando um painel ancorado à direita
            egui::SidePanel::right("coluna_direita")
                .frame(egui::Frame::none().inner_margin(egui::Margin::symmetric(24.0, 24.0))) // Topo e laterais
                .resizable(false)
                .exact_width(320.0)
                .show_inside(ui, |ui| {
                    
                    // Botão de fechar (Apenas na coluna direita para ficar no topo direito)
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                        if ui.button("❌").clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                    ui.add_space(10.0);

                    frame_cards.show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.heading(egui::RichText::new("Leitor de Tela").strong());
                        ui.add_space(16.0);
                        
                        ui.horizontal(|ui| {
                            ui.label("  [ 📱 ]  "); 
                            ui.vertical(|ui| {
                                let _ = ui.button("   Capturar Tela   ");
                                let _ = ui.button("  Selecionar Área  ");
                            });
                        });
                    });

                    ui.add_space(12.0);

                    frame_cards.show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.heading(egui::RichText::new("Texto para Voz").strong());
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label("Ligado");
                                ui.visuals_mut().selection.bg_fill = egui::Color32::from_rgb(0, 197, 175);
                                ui.checkbox(&mut self.leitor_ligado, ""); 
                            });
                        });
                        ui.add_space(8.0);
                        
                        ui.label("Volume");
                        ui.visuals_mut().selection.bg_fill = egui::Color32::WHITE;
                        
                        // Uso de width fixo baseado no painel pai, totalmente seguro contra o crash.
                        let w = ui.available_width().max(10.0);
                        ui.add_sized([w, 22.0], egui::Slider::new(&mut self.contraste, 0.0..=100.0).show_value(false));
                    });
                });

            // ==========================================
            // 3. COLUNA DE CONTEÚDO (CENTRO / SLIDERS)
            // ==========================================
            // O painel central ocupa o espaço que sobrou entre a esquerda e a direita.
            egui::CentralPanel::default()
                .frame(egui::Frame::none().inner_margin(egui::Margin { left: 24.0, right: 0.0, top: 24.0, bottom: 24.0 }))
                .show_inside(ui, |ui| {
                    
                    ui.visuals_mut().selection.bg_fill = egui::Color32::WHITE;

                    // Alinha o topo dos cards da coluna do meio para ficarem na mesma altura da coluna da direita (abaixo do X de fechar)
                    ui.add_space(42.0); 

                    frame_cards.show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.heading(egui::RichText::new("Contraste").strong());
                        ui.add_space(4.0);
                        let w = ui.available_width().max(10.0);
                        ui.add_sized([w, 22.0], egui::Slider::new(&mut self.contraste, 0.0..=100.0).show_value(false));
                    });

                    ui.add_space(12.0);
                    
                    frame_cards.show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.heading(egui::RichText::new("Saturação").strong());
                        ui.add_space(4.0);
                        let w = ui.available_width().max(10.0);
                        ui.add_sized([w, 22.0], egui::Slider::new(&mut self.saturacao, 0.0..=100.0).show_value(false));
                    });

                    ui.add_space(12.0);

                    frame_cards.show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.heading(egui::RichText::new("Brilho").strong());
                        ui.add_space(4.0);
                        let w = ui.available_width().max(10.0);
                        ui.add_sized([w, 22.0], egui::Slider::new(&mut self.brilho, 0.0..=100.0).show_value(false));
                    });
                    
                    ui.add_space(12.0);

                    frame_cards.show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.heading(egui::RichText::new("Filtro").strong());
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                egui::ComboBox::from_id_salt("cb_filtro")
                                    .selected_text("Deuteranopia")
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(&mut self.filtro_selecionado, 0, "Deuteranopia");
                                        ui.selectable_value(&mut self.filtro_selecionado, 1, "Protanopia");
                                    });
                            });
                        });
                    });
                });
        });
    }
}