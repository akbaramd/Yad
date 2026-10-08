use std::fs;

use anyhow::{Context, Result, bail};
use fastembed::{
    EmbeddingModel, InitOptionsUserDefined, Pooling, QuantizationMode, TextEmbedding,
    TextInitOptions, TokenizerFiles, UserDefinedEmbeddingModel,
};

use crate::model_cache;

#[derive(Debug, Clone, Copy)]
enum InputMode {
    Plain,
    E5,
}

pub struct EmbeddingService {
    model: TextEmbedding,
    input_mode: InputMode,
}

impl EmbeddingService {
    pub fn new(model_name: &str) -> Result<Self> {
        match model_name.to_ascii_lowercase().as_str() {
            "multilingual-e5-small-int8" => {
                let bundle = model_cache::ensure_default_bundle()?;

                let tokenizer_files = TokenizerFiles {
                    tokenizer_file: fs::read(&bundle.tokenizer)
                        .context("failed to read tokenizer.json")?,
                    config_file: fs::read(&bundle.config)
                        .context("failed to read embedding config.json")?,
                    special_tokens_map_file: fs::read(&bundle.special_tokens_map)
                        .context("failed to read special_tokens_map.json")?,
                    tokenizer_config_file: fs::read(&bundle.tokenizer_config)
                        .context("failed to read tokenizer_config.json")?,
                };

                let user_model = UserDefinedEmbeddingModel::new(
                    fs::read(&bundle.model).context("failed to read local embedding ONNX model")?,
                    tokenizer_files,
                )
                .with_pooling(Pooling::Mean)
                .with_quantization(QuantizationMode::Static);

                let options = InitOptionsUserDefined::new().with_intra_threads(inference_threads());

                let model = TextEmbedding::try_new_from_user_defined(user_model, options)
                    .context("failed to initialize cached local embedding model")?;

                Ok(Self {
                    model,
                    input_mode: InputMode::E5,
                })
            }
            "paraphrase-multilingual-minilm-l12-v2" => Self::from_fastembed(
                EmbeddingModel::ParaphraseMLMiniLML12V2,
                InputMode::Plain,
                model_name,
            ),
            "multilingual-e5-small" => Self::from_fastembed(
                EmbeddingModel::MultilingualE5Small,
                InputMode::E5,
                model_name,
            ),
            other => bail!(
                "unsupported embedding model '{}'; supported: multilingual-e5-small-int8, paraphrase-multilingual-minilm-l12-v2, multilingual-e5-small",
                other
            ),
        }
    }

    fn from_fastembed(
        model_id: EmbeddingModel,
        input_mode: InputMode,
        model_name: &str,
    ) -> Result<Self> {
        let options = TextInitOptions::new(model_id)
            .with_cache_dir(model_cache_root())
            .with_show_download_progress(false)
            .with_intra_threads(inference_threads());

        let model = TextEmbedding::try_new(options).with_context(|| {
            format!(
                "failed to initialize local embedding model '{}'",
                model_name
            )
        })?;

        Ok(Self { model, input_mode })
    }

    pub fn embed_passages(&mut self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let inputs = texts
            .iter()
            .map(|value| match self.input_mode {
                InputMode::Plain => value.clone(),
                InputMode::E5 => format!("passage: {}", value),
            })
            .collect::<Vec<_>>();

        self.model
            .embed(inputs, None)
            .context("failed to generate passage embeddings")
    }

    pub fn embed_query(&mut self, query: &str) -> Result<Vec<f32>> {
        let input = match self.input_mode {
            InputMode::Plain => query.to_string(),
            InputMode::E5 => format!("query: {}", query),
        };

        let mut result = self
            .model
            .embed(vec![input], None)
            .context("failed to generate query embedding")?;

        result
            .pop()
            .ok_or_else(|| anyhow::anyhow!("embedding model returned no vector"))
    }
}

fn inference_threads() -> usize {
    std::thread::available_parallelism()
        .map(|value| value.get().clamp(1, 4))
        .unwrap_or(2)
}

fn model_cache_root() -> std::path::PathBuf {
    if let Some(base) = std::env::var_os("LOCALAPPDATA") {
        std::path::PathBuf::from(base).join("Yad").join("models")
    } else if let Some(home) = std::env::var_os("HOME") {
        std::path::PathBuf::from(home)
            .join(".cache")
            .join("yad")
            .join("models")
    } else {
        std::env::temp_dir().join("yad").join("models")
    }
}
