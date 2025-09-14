use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

use crate::services::llm::interface::{UnifiedLlmResponse, TokenUsage, ResponseMetadata};
use crate::errors::AppError;

/// Common response formatting and normalization layer
#[derive(Debug, Clone)]
pub struct ResponseFormatter {
    /// Whether to strip markdown formatting
    pub strip_markdown: bool,
    
    /// Whether to extract code blocks
    pub extract_code_blocks: bool,
    
    /// Whether to normalize whitespace
    pub normalize_whitespace: bool,
    
    /// Whether to extract structured data
    pub extract_structured_data: bool,
}

impl Default for ResponseFormatter {
    fn default() -> Self {
        Self {
            strip_markdown: false,
            extract_code_blocks: false,
            normalize_whitespace: true,
            extract_structured_data: false,
        }
    }
}

/// Formatted response with extracted elements
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormattedResponse {
    /// The main formatted content
    pub content: String,
    
    /// Extracted code blocks
    pub code_blocks: Vec<CodeBlock>,
    
    /// Extracted structured data (JSON, YAML, etc.)
    pub structured_data: Vec<StructuredData>,
    
    /// Extracted thinking/reasoning
    pub thinking: Option<String>,
    
    /// Metadata from original response
    pub metadata: ResponseMetadata,
    
    /// Original response
    pub original: UnifiedLlmResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeBlock {
    pub language: Option<String>,
    pub code: String,
    pub start_line: usize,
    pub end_line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredData {
    pub format: DataFormat,
    pub content: String,
    pub parsed: Option<Value>,
    pub start_line: usize,
    pub end_line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DataFormat {
    Json,
    Yaml,
    Toml,
    Xml,
    Csv,
    Unknown,
}

impl ResponseFormatter {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn strip_markdown(mut self, strip: bool) -> Self {
        self.strip_markdown = strip;
        self
    }
    
    pub fn extract_code_blocks(mut self, extract: bool) -> Self {
        self.extract_code_blocks = extract;
        self
    }
    
    pub fn normalize_whitespace(mut self, normalize: bool) -> Self {
        self.normalize_whitespace = normalize;
        self
    }
    
    pub fn extract_structured_data(mut self, extract: bool) -> Self {
        self.extract_structured_data = extract;
        self
    }
    
    /// Format a response with the configured options
    pub fn format(&self, response: UnifiedLlmResponse) -> Result<FormattedResponse, AppError> {
        let mut content = response.content.clone();
        let mut code_blocks = Vec::new();
        let mut structured_data = Vec::new();
        
        // Extract code blocks first
        if self.extract_code_blocks {
            let (new_content, blocks) = self.extract_code_blocks_from_text(&content)?;
            content = new_content;
            code_blocks = blocks;
        }
        
        // Extract structured data
        if self.extract_structured_data {
            let (new_content, data) = self.extract_structured_data_from_text(&content)?;
            content = new_content;
            structured_data = data;
        }
        
        // Strip markdown if requested
        if self.strip_markdown {
            content = self.strip_markdown_formatting(&content);
        }
        
        // Normalize whitespace if requested
        if self.normalize_whitespace {
            content = self.normalize_whitespace_in_text(&content);
        }
        
        Ok(FormattedResponse {
            content,
            code_blocks,
            structured_data,
            thinking: response.thinking.clone(),
            metadata: response.metadata.clone(),
            original: response,
        })
    }
    
    /// Extract code blocks from text
    fn extract_code_blocks_from_text(&self, text: &str) -> Result<(String, Vec<CodeBlock>), AppError> {
        let mut blocks = Vec::new();
        let mut result = String::new();
        let lines: Vec<&str> = text.lines().collect();
        let mut i = 0;
        let mut current_line = 1;
        
        while i < lines.len() {
            let line = lines[i];
            
            // Check for fenced code block
            if line.starts_with("```") {
                let language = if line.len() > 3 {
                    Some(line[3..].trim().to_string())
                } else {
                    None
                };
                
                let start_line = current_line + 1;
                let mut code_content = String::new();
                let mut end_found = false;
                i += 1;
                current_line += 1;
                
                // Collect code block content
                while i < lines.len() {
                    if lines[i] == "```" {
                        end_found = true;
                        break;
                    }
                    code_content.push_str(lines[i]);
                    code_content.push('\n');
                    i += 1;
                    current_line += 1;
                }
                
                if end_found {
                    blocks.push(CodeBlock {
                        language,
                        code: code_content.trim_end().to_string(),
                        start_line,
                        end_line: current_line - 1,
                    });
                    
                    // Add placeholder in result
                    result.push_str(&format!("[CODE_BLOCK_{}]\n", blocks.len() - 1));
                } else {
                    // Not a complete code block, add back to result
                    result.push_str("```");
                    if let Some(lang) = &language {
                        result.push_str(lang);
                    }
                    result.push('\n');
                    result.push_str(&code_content);
                }
            } else {
                result.push_str(line);
                result.push('\n');
            }
            
            i += 1;
            current_line += 1;
        }
        
        Ok((result, blocks))
    }
    
    /// Extract structured data from text
    fn extract_structured_data_from_text(&self, text: &str) -> Result<(String, Vec<StructuredData>), AppError> {
        let mut data = Vec::new();
        let mut result = text.to_string();
        
        // Simple JSON extraction
        if let Some((json_str, start, end)) = self.extract_json_from_text(text) {
            let parsed = serde_json::from_str(&json_str).ok();
            data.push(StructuredData {
                format: DataFormat::Json,
                content: json_str,
                parsed,
                start_line: start,
                end_line: end,
            });
        }
        
        // TODO: Add YAML, XML, CSV extraction
        
        Ok((result, data))
    }
    
    /// Extract JSON from text
    fn extract_json_from_text(&self, text: &str) -> Option<(String, usize, usize)> {
        let lines: Vec<&str> = text.lines().collect();
        let mut brace_count = 0;
        let mut start_line = None;
        let mut json_content = String::new();
        
        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            
            if trimmed.starts_with('{') && start_line.is_none() {
                start_line = Some(i + 1);
                json_content.clear();
            }
            
            if start_line.is_some() {
                json_content.push_str(line);
                json_content.push('\n');
                
                // Count braces
                for ch in line.chars() {
                    match ch {
                        '{' => brace_count += 1,
                        '}' => brace_count -= 1,
                        _ => {}
                    }
                }
                
                if brace_count == 0 {
                    return Some((json_content.trim().to_string(), start_line.unwrap(), i + 1));
                }
            }
        }
        
        None
    }
    
    /// Strip markdown formatting
    fn strip_markdown_formatting(&self, text: &str) -> String {
        let mut result = text.to_string();
        
        // Remove headers
        result = regex::Regex::new(r"#+\s*").unwrap()
            .replace_all(&result, "").to_string();
        
        // Remove bold/italic
        result = regex::Regex::new(r"\*\*([^*]+)\*\*").unwrap()
            .replace_all(&result, "$1").to_string();
        result = regex::Regex::new(r"\*([^*]+)\*").unwrap()
            .replace_all(&result, "$1").to_string();
        
        // Remove links
        result = regex::Regex::new(r"\[([^\]]+)\]\([^)]+\)").unwrap()
            .replace_all(&result, "$1").to_string();
        
        // Remove inline code
        result = regex::Regex::new(r"`([^`]+)`").unwrap()
            .replace_all(&result, "$1").to_string();
        
        result
    }
    
    /// Normalize whitespace
    fn normalize_whitespace_in_text(&self, text: &str) -> String {
        // Collapse multiple spaces into single space
        let result = regex::Regex::new(r" +").unwrap()
            .replace_all(text, " ");
        
        // Collapse multiple newlines into double newlines
        let result = regex::Regex::new(r"\n{3,}").unwrap()
            .replace_all(&result, "\n\n");
        
        result.trim().to_string()
    }
}

/// Response analysis utilities
pub struct ResponseAnalyzer;

impl ResponseAnalyzer {
    /// Analyze response quality
    pub fn analyze_quality(response: &UnifiedLlmResponse) -> QualityMetrics {
        QualityMetrics {
            content_length: response.content.len(),
            has_thinking: response.thinking.is_some(),
            token_efficiency: Self::calculate_token_efficiency(response),
            estimated_readability: Self::estimate_readability(&response.content),
            contains_code: Self::contains_code(&response.content),
            contains_structured_data: Self::contains_structured_data(&response.content),
        }
    }
    
    fn calculate_token_efficiency(response: &UnifiedLlmResponse) -> Option<f64> {
        if let (Some(total), Some(completion)) = (response.usage.total_tokens, response.usage.completion_tokens) {
            Some(completion as f64 / total as f64)
        } else {
            None
        }
    }
    
    fn estimate_readability(text: &str) -> ReadabilityScore {
        let sentences = text.split('.').count();
        let words = text.split_whitespace().count();
        let avg_words_per_sentence = if sentences > 0 { words as f64 / sentences as f64 } else { 0.0 };
        
        if avg_words_per_sentence < 15.0 {
            ReadabilityScore::High
        } else if avg_words_per_sentence < 25.0 {
            ReadabilityScore::Medium
        } else {
            ReadabilityScore::Low
        }
    }
    
    fn contains_code(text: &str) -> bool {
        text.contains("```") || text.contains("function") || text.contains("class") || text.contains("def ")
    }
    
    fn contains_structured_data(text: &str) -> bool {
        text.contains("{") && text.contains("}") || text.contains("---") || text.contains("<xml")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QualityMetrics {
    pub content_length: usize,
    pub has_thinking: bool,
    pub token_efficiency: Option<f64>,
    pub estimated_readability: ReadabilityScore,
    pub contains_code: bool,
    pub contains_structured_data: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ReadabilityScore {
    High,
    Medium,
    Low,
}
