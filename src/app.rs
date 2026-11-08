// src/app.rs
use egui::Context;
use crate::filtros;
use crate::audio::GerenciadorAudio;
use crate::leitor::GerenciadorLeitor;
use crate::lupa::GerenciadorLupa;
use crate::config::AppConfig;

#[derive(PartialEq)]
pub enum AbaMenu {
    Filtros,
    Lupa,
    Leitor,
    Audio,
    Config,
}

pub fn aplicar_tema_ghost(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    let mut visuals = egui::Visuals::dark();
    
    // 🎨 1. CORES E TRANSPARÊNCIA
    // Fundo da janela: (R, G, B, Opacidade) -> 180 = Bem transparente, 255 = Sólido
    visuals.window_fill = egui::Color32::from_rgba_unmultiplied(87, 101, 116, 220);
    
    // Cor da linha de borda da janela
    visuals.window_stroke = egui::Stroke::new(1.5, egui::Color32::from_rgb(80, 80, 100));
    
    // Cor de destaque (Acento) - Mude para verde limão, vermelho sangue, azul ciano...
    let cor_destaque = egui::Color32::from_rgb(0, 200, 255); // Ex: Azul Ciano
    visuals.selection.bg_fill = cor_destaque;
    
    // 📐 2. ARREDONDAMENTO DAS BORDAS (0.0 = Quadrado, 12.0 = Redondo)
    let raio_borda = 8.0; 
    visuals.window_rounding = egui::Rounding::same(raio_borda);
    visuals.widgets.noninteractive.rounding = egui::Rounding::same(raio_borda);
    visuals.widgets.inactive.rounding = egui::Rounding::same(raio_borda);
    visuals.widgets.hovered.rounding = egui::Rounding::same(raio_borda);
    visuals.widgets.active.rounding = egui::Rounding::same(raio_borda);

    // Efeito de "brilho" nas bordas quando você interage
    visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, cor_destaque);
    visuals.widgets.active.bg_stroke = egui::Stroke::new(2.0, cor_destaque);

    // 📏 3. ESPAÇAMENTOS E MARGENS
    // Distância entre os botões e os textos (X, Y)
    style.spacing.item_spacing = egui::Vec2::new(10.0, 12.0); 
    // Largura das barrinhas de arrastar (sliders)
    style.spacing.slider_width = 250.0;
    // Margem interna geral da janela
    style.spacing.window_margin = egui::Margin::same(16.0); 
    // Espaçamento interno dos botões (deixa eles mais "gordinhos" e fáceis de clicar)
    style.spacing.button_padding = egui::Vec2::new(12.0, 8.0);

    // 🔠 4. TAMANHO DAS FONTES
    use egui::{FontId, TextStyle};
    style.text_styles = [
        (TextStyle::Heading, FontId::proportional(26.0)), // Títulos principais
        (TextStyle::Body, FontId::proportional(16.0)),    // Textos normais
        (TextStyle::Button, FontId::proportional(16.0)),  // Textos dentro de botões
        (TextStyle::Small, FontId::proportional(12.0)),   // Textos menores/dicas
    ].into();

    style.visuals = visuals;
    ctx.set_style(style);
}

pub struct GhostOverlayApp {
    pub config: AppConfig,
    pub lupa: GerenciadorLupa,
    pub leitor: GerenciadorLeitor,
    pub audio: GerenciadorAudio,
    pub modo_menu_interativo: bool,
    pub aba_atual: AbaMenu,
    pub deve_fechar: bool,
}

impl GhostOverlayApp {
    pub fn new(ctx: &Context) -> Self {
        aplicar_tema_ghost(ctx);
        
        let config = AppConfig::carregar();
        
        let mut audio = GerenciadorAudio::new();
        audio.set_ativo(config.limitador_ligado);
        audio.set_limite(config.limite_volume);

        let mut leitor = GerenciadorLeitor::new(ctx.clone());
        leitor.ativo = config.leitor_ativo;
        leitor.tts_volume = config.tts_volume;
        leitor.tts_velocidade = config.tts_velocidade;
        leitor.tts_pitch = config.tts_pitch;
        leitor.leitura_periodica = config.leitura_periodica;
        leitor.intervalo_leitura = config.intervalo_leitura;
        leitor.leitura_mouse = config.leitura_mouse;
        leitor.largura_area_mouse = config.largura_area_mouse;
        leitor.altura_area_mouse = config.altura_area_mouse;
        leitor.atualizar_config_tts();

        let mut lupa = GerenciadorLupa::new(ctx.clone());
        lupa.ativo = config.lupa_ativa;
        lupa.modo_mouse = config.lupa_modo_mouse;
        lupa.fator_zoom = config.lupa_fator_zoom;
        lupa.tamanho_janela = config.lupa_tamanho_janela;
        lupa.pixel_art = config.lupa_pixel_art;

        filtros::atualizar_filtros(config.filtro_atual, config.brilho, config.contraste, config.saturacao, config.intensidade);

        Self {
            config,
            lupa,
            leitor,
            audio,
            modo_menu_interativo: true,
            aba_atual: AbaMenu::Filtros,
            deve_fechar: false,
        }
    }

    pub fn atualizar_tick(&mut self, ctx: &Context) {
        self.leitor.atualizar_tick(ctx);
        self.lupa.atualizar_tick(ctx);
    }

    pub fn renderizar_ui(&mut self, ctx: &Context) {
        if self.modo_menu_interativo || self.leitor.modo_selecao || self.lupa.selecionando_centro {
            if self.modo_menu_interativo {
                egui::Window::new("GhostOverlay_Principal")
                    .title_bar(false)
                    .resizable(false)
                    .collapsible(false)
                    .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                    .fixed_size(egui::Vec2::new(600.0, 450.0))
                    .show(ctx, |ui| {
                        
                        ui.horizontal(|ui| {
                            ui.heading("👻 GhostOverlay");
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.button("❌ Fechar Menu (Shift+F11)").clicked() {
                                    self.modo_menu_interativo = false;
                                }
                            });
                        });
                        ui.add_space(4.0);
                        ui.separator();
                        
                        ui.horizontal(|ui| {
                            ui.selectable_value(&mut self.aba_atual, AbaMenu::Filtros, "🎨 Filtros Visuais");
                            ui.selectable_value(&mut self.aba_atual, AbaMenu::Lupa, "🔍 Lupa");
                            ui.selectable_value(&mut self.aba_atual, AbaMenu::Leitor, "🗣️ Leitor de Tela");
                            ui.selectable_value(&mut self.aba_atual, AbaMenu::Audio, "🔊 Proteção de Áudio");
                            ui.selectable_value(&mut self.aba_atual, AbaMenu::Config, "⚙️ Sistema");
                        });
                        ui.separator();
                        ui.add_space(8.0);

                        egui::ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                match self.aba_atual {
                                    AbaMenu::Filtros => {
                                        ui.heading("Ajustes de Vídeo e Daltonismo");
                                        ui.add_space(8.0);
                                        
                                        let mut mudou_video = false;
                                        egui::ComboBox::from_label("Filtro Ativo")
                                            .selected_text(match self.config.filtro_atual {
                                                filtros::ModoDaltonismo::Desativado => "Nenhum (Normal)",
                                                filtros::ModoDaltonismo::Protanopia => "Protanopia (Vermelho)",
                                                filtros::ModoDaltonismo::Deuteranopia => "Deuteranopia (Verde)",
                                                filtros::ModoDaltonismo::Tritanopia => "Tritanopia (Azul)",
                                                filtros::ModoDaltonismo::Acromatopsia => "Acromatopsia (Preto e Branco)",
                                            })
                                            .show_ui(ui, |ui| {
                                                mudou_video |= ui.selectable_value(&mut self.config.filtro_atual, filtros::ModoDaltonismo::Desativado, "Nenhum (Normal)").changed();
                                                mudou_video |= ui.selectable_value(&mut self.config.filtro_atual, filtros::ModoDaltonismo::Protanopia, "Protanopia (Vermelho)").changed();
                                                mudou_video |= ui.selectable_value(&mut self.config.filtro_atual, filtros::ModoDaltonismo::Deuteranopia, "Deuteranopia (Verde)").changed();
                                                mudou_video |= ui.selectable_value(&mut self.config.filtro_atual, filtros::ModoDaltonismo::Tritanopia, "Tritanopia (Azul)").changed();
                                                mudou_video |= ui.selectable_value(&mut self.config.filtro_atual, filtros::ModoDaltonismo::Acromatopsia, "Acromatopsia (Preto e Branco)").changed();
                                            });

                                        if self.config.filtro_atual != filtros::ModoDaltonismo::Desativado {
                                            ui.add_space(4.0);
                                            mudou_video |= ui.add(egui::Slider::new(&mut self.config.intensidade, 0.0..=1.0).text("Intensidade do Filtro").custom_formatter(|v, _| format!("{:.0}%", v * 100.0))).changed();
                                        }

                                        ui.add_space(6.0);
                                        mudou_video |= ui.add(egui::Slider::new(&mut self.config.brilho, -0.5..=0.5).text("Brilho")).changed();
                                        mudou_video |= ui.add(egui::Slider::new(&mut self.config.contraste, 0.3..=2.0).text("Contraste")).changed();
                                        mudou_video |= ui.add(egui::Slider::new(&mut self.config.saturacao, 0.0..=3.0).text("Saturação")).changed();

                                        if mudou_video {
                                            filtros::atualizar_filtros(self.config.filtro_atual, self.config.brilho, self.config.contraste, self.config.saturacao, self.config.intensidade);
                                        }
                                    }
                                    AbaMenu::Lupa => {
                                        if self.lupa.exibir_ui(ui) {
                                            self.modo_menu_interativo = false; 
                                        }
                                    }
                                    AbaMenu::Leitor => {
                                        if self.leitor.exibir_ui(ui, ctx) {
                                            self.modo_menu_interativo = false; 
                                        }
                                    }
                                    AbaMenu::Audio => {
                                        ui.heading("Proteção Auditiva");
                                        ui.add_space(8.0);
                                        if ui.checkbox(&mut self.config.limitador_ligado, "Ativar Limitador de Áudio Reativo").changed() {
                                            self.audio.set_ativo(self.config.limitador_ligado);
                                        }

                                        if self.config.limitador_ligado {
                                            ui.small("Abaixa o som do PC automaticamente se passar do teto estipulado.");
                                            if ui.add(egui::Slider::new(&mut self.config.limite_volume, 0.05..=0.95).text("Teto de Volume").custom_formatter(|v, _| format!("{:.0}% do Pico", v * 100.0))).changed() {
                                                self.audio.set_limite(self.config.limite_volume);
                                            }
                                        }
                                    }
                                    AbaMenu::Config => {
                                        ui.heading("Gerenciamento do Software");
                                        ui.add_space(8.0);
                                        ui.label("Caminho do arquivo de configuração:");
                                        ui.monospace("%APPDATA%/GhostOverlay/config.json");
                                        ui.add_space(10.0);

                                        ui.horizontal(|ui| {
                                            if ui.button("⚠️ Resetar Tudo").clicked() {
                                                self.config = AppConfig::default(); 
                                                filtros::atualizar_filtros(self.config.filtro_atual, self.config.brilho, self.config.contraste, self.config.saturacao, self.config.intensidade);
                                                self.audio.set_ativo(false);
                                                self.leitor.ativo = false;
                                                self.leitor.regiao_selecionada = None;
                                                self.leitor.texto_extraido = String::from("Nenhum texto processado ainda.");
                                            }
                                            
                                            ui.add_space(20.0);
                                            
                                            if ui.button("🛑 Fechar Software").clicked() {
                                                self.deve_fechar = true;
                                            }
                                        });
                                    }
                                }
                            });
                    });
            } else {
                self.leitor.executar_selecao_invisivel(ctx);
                self.lupa.executar_selecao_centro(ctx);
                
                if !self.leitor.modo_selecao && !self.lupa.selecionando_centro {
                    self.modo_menu_interativo = true;
                }
            }
        }

        self.leitor.renderizar_overlay_debug(ctx);
        self.lupa.renderizar_janela(ctx);
    }

    pub fn salvar_e_fechar(&mut self) {
        self.config.leitor_ativo = self.leitor.ativo;
        self.config.tts_volume = self.leitor.tts_volume;
        self.config.tts_velocidade = self.leitor.tts_velocidade;
        self.config.tts_pitch = self.leitor.tts_pitch;
        self.config.leitura_periodica = self.leitor.leitura_periodica;
        self.config.intervalo_leitura = self.leitor.intervalo_leitura;
        self.config.leitura_mouse = self.leitor.leitura_mouse;
        self.config.largura_area_mouse = self.leitor.largura_area_mouse;
        self.config.altura_area_mouse = self.leitor.altura_area_mouse;
        
        self.config.lupa_ativa = self.lupa.ativo;
        self.config.lupa_modo_mouse = self.lupa.modo_mouse;
        self.config.lupa_fator_zoom = self.lupa.fator_zoom;
        self.config.lupa_tamanho_janela = self.lupa.tamanho_janela;
        self.config.lupa_pixel_art = self.lupa.pixel_art;

        self.config.salvar();

        filtros::atualizar_filtros(filtros::ModoDaltonismo::Desativado, 0.0, 1.0, 1.0, 1.0);
        self.audio.set_ativo(false);
    }
}