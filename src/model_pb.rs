// SPDX-License-Identifier: Apache-2.0
// SPDX-FileCopyrightText: 2024 Hyperpolymath

//! Protobuf message types for the analysis pipeline
//!
//! These types match the schema in proto/analysis.proto.
//! Generated via prost derive macros (no protoc required at build time).

use prost::Message;

/// Input message for content analysis
#[derive(Clone, PartialEq, Message)]
pub struct AnalysisInput {
    #[prost(string, tag = "1")]
    pub content_hash: String,

    #[prost(string, tag = "2")]
    pub content_text: String,

    #[prost(string, tag = "3")]
    pub source_id: String,

    #[prost(string, tag = "4")]
    pub image_url: String,
}

/// Neural feature outputs from ONNX inference
#[derive(Clone, PartialEq, Message)]
pub struct NeuralFeatures {
    #[prost(float, tag = "1")]
    pub fakeness_score: f32,

    #[prost(float, tag = "2")]
    pub emotion_score: f32,

    #[prost(bool, tag = "3")]
    pub visual_artifact: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analysis_input_roundtrip() {
        let input = AnalysisInput {
            content_hash: "abc123".to_string(),
            content_text: "Test content".to_string(),
            source_id: "source-1".to_string(),
            image_url: "https://example.com/img.png".to_string(),
        };

        // Encode
        let mut buf = Vec::new();
        input.encode(&mut buf).unwrap();

        // Decode
        let decoded = AnalysisInput::decode(&buf[..]).unwrap();

        assert_eq!(input, decoded);
    }

    #[test]
    fn test_neural_features_roundtrip() {
        let features = NeuralFeatures {
            fakeness_score: 0.75,
            emotion_score: 0.42,
            visual_artifact: true,
        };

        let mut buf = Vec::new();
        features.encode(&mut buf).unwrap();

        let decoded = NeuralFeatures::decode(&buf[..]).unwrap();

        assert_eq!(features, decoded);
    }
}
