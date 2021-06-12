use crate::{Message, MessageSender};

pub fn install(tx: MessageSender) {
    use tracing_error::ErrorLayer;
    use tracing_subscriber::prelude::*;
    use tracing_subscriber::{fmt, EnvFilter};

    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
        .unwrap();
    let fmt_layer = fmt::layer()
        .with_target(false)
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::ENTER);

    struct ForwardLayer {
        tx: MessageSender,
    }

    impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for ForwardLayer {
        fn on_event(
            &self,
            event: &tracing::Event<'_>,
            _ctx: tracing_subscriber::layer::Context<'_, S>,
        ) {
            struct V {
                ui: bool,
                message: Option<String>,
            }

            impl tracing::field::Visit for V {
                fn record_debug(
                    &mut self,
                    field: &tracing::field::Field,
                    value: &dyn std::fmt::Debug,
                ) {
                    if field.name() == "message" {
                        self.message = Some(format!("{:?}", value));
                    }
                }

                fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
                    if field.name() == "ui" {
                        self.ui = value;
                    }
                }

                fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
                    if field.name() == "message" {
                        self.message = Some(value.to_owned());
                    }
                }
            }

            let mut visit = V {
                ui: false,
                message: None,
            };

            event.record(&mut visit);

            if visit.ui {
                if let Some(message) = visit.message {
                    self.tx.send(Message::Status(message)).unwrap();
                }
            }
        }
    }

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .with(ForwardLayer { tx })
        .with(ErrorLayer::default())
        .init();
}
