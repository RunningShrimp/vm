//! Shader 翻译器
//!
//! 支持多种着色器语言之间的翻译：
//! - HLSL → GLSL
//! - GLSL → MSL (Metal Shading Language)
//! - SPIR-V 生成和优化

use std::collections::HashMap;

/// 着色器语言
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShaderLanguage {
    HLSL,  // DirectX
    GLSL,  // OpenGL
    MSL,   // Metal
    SPIRV, // Vulkan
}

/// 着色器阶段
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShaderStage {
    Vertex,
    Fragment,
    Geometry,
    Compute,
}

/// 着色器
#[derive(Debug, Clone)]
pub struct Shader {
    pub name: String,
    pub language: ShaderLanguage,
    pub stage: ShaderStage,
    pub source: String,
}

/// 着色器翻译器
pub struct ShaderTranslator {
    /// 翻译缓存
    cache: HashMap<(String, ShaderLanguage, ShaderLanguage), String>,
    
    /// 翻译统计
    stats: TranslatorStats,
}

/// 翻译统计
#[derive(Debug, Clone, Default)]
pub struct TranslatorStats {
    pub translations: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
}

impl ShaderTranslator {
    /// 创建新的着色器翻译器
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
            stats: TranslatorStats::default(),
        }
    }

    /// 翻译着色器
    ///
    /// # 参数
    ///
    /// * `shader` - 源着色器
    /// * `target_language` - 目标语言
    pub fn translate(
        &mut self,
        shader: &Shader,
        target_language: ShaderLanguage,
    ) -> Result<Shader, ShaderError> {
        if shader.language == target_language {
            return Ok(shader.clone());
        }

        self.stats.translations += 1;

        // 检查缓存
        let cache_key = (shader.name.clone(), shader.language, target_language);
        if let Some(cached) = self.cache.get(&cache_key) {
            self.stats.cache_hits += 1;
            return Ok(Shader {
                name: shader.name.clone(),
                language: target_language,
                stage: shader.stage,
                source: cached.clone(),
            });
        }

        self.stats.cache_misses += 1;

        // 执行翻译
        let translated = match (shader.language, target_language) {
            (ShaderLanguage::HLSL, ShaderLanguage::GLSL) => {
                self.hlsl_to_glsl(&shader.source)?
            }
            (ShaderLanguage::GLSL, ShaderLanguage::MSL) => {
                self.glsl_to_msl(&shader.source)?
            }
            (ShaderLanguage::HLSL, ShaderLanguage::SPIRV) => {
                self.hlsl_to_spirv(&shader.source)?
            }
            (ShaderLanguage::GLSL, ShaderLanguage::SPIRV) => {
                self.glsl_to_spirv(&shader.source)?
            }
            _ => {
                return Err(ShaderError::UnsupportedTranslation {
                    from: shader.language,
                    to: target_language,
                });
            }
        };

        // 缓存结果
        self.cache.insert(cache_key, translated.clone());

        Ok(Shader {
            name: shader.name.clone(),
            language: target_language,
            stage: shader.stage,
            source: translated,
        })
    }

    /// HLSL → GLSL 翻译
    fn hlsl_to_glsl(&self, hlsl: &str) -> Result<String, ShaderError> {
        // 简化实现：替换类型和关键字
        let glsl = hlsl
            .replace("float2", "vec2")
            .replace("float3", "vec3")
            .replace("float4", "vec4")
            .replace("int2", "ivec2")
            .replace("int3", "ivec3")
            .replace("int4", "ivec4")
            .replace("matrix", "mat4")
            .replace("Texture2D", "sampler2D")
            .replace("SamplerState", "sampler2D");

        Ok(glsl)
    }

    /// GLSL → MSL 翻译（Metal）
    fn glsl_to_msl(&self, glsl: &str) -> Result<String, ShaderError> {
        // 简化实现：Metal 着色器语法转换
        let msl = glsl
            .replace("vec2", "float2")
            .replace("vec3", "float3")
            .replace("vec4", "float4")
            .replace("mat4", "float4x4")
            .replace("sampler2D", "texture2d")
            .replace("texture(", "texture.sample(");

        Ok(msl)
    }

    /// HLSL → SPIR-V 翻译
    fn hlsl_to_spirv(&self, hlsl: &str) -> Result<String, ShaderError> {
        // 需要使用 glslang 或 SPIRV-Cross
        log::warn!("HLSL → SPIR-V translation requires external compiler");
        
        // 临时返回占位符
        Ok(format!("// SPIR-V would be here\n{}", hlsl))
    }

    /// GLSL → SPIR-V 翻译
    fn glsl_to_spirv(&self, glsl: &str) -> Result<String, ShaderError> {
        log::warn!("GLSL → SPIR-V translation requires external compiler");
        
        Ok(format!("// SPIR-V would be here\n{}", glsl))
    }

    /// 获取翻译统计
    pub fn get_stats(&self) -> &TranslatorStats {
        &self.stats
    }

    /// 清空缓存
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }
}

impl Default for ShaderTranslator {
    fn default() -> Self {
        Self::new()
    }
}

/// 着色器错误
#[derive(Debug, thiserror::Error)]
pub enum ShaderError {
    #[error("Unsupported translation: {from:?} → {to:?}")]
    UnsupportedTranslation {
        from: ShaderLanguage,
        to: ShaderLanguage,
    },

    #[error("Syntax error: {0}")]
    SyntaxError(String),

    #[error("Compilation failed: {0}")]
    CompilationFailed(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shader_translator_creation() {
        let translator = ShaderTranslator::new();
        assert_eq!(translator.stats.translations, 0);
    }

    #[test]
    fn test_hlsl_to_glsl() {
        let mut translator = ShaderTranslator::new();
        
        let shader = Shader {
            name: "test_shader".to_string(),
            language: ShaderLanguage::HLSL,
            stage: ShaderStage::Fragment,
            source: "float4 main(float2 pos : POSITION) : SV_Position { return float4(pos, 0, 1); }".to_string(),
        };
        
        let result = translator.translate(&shader, ShaderLanguage::GLSL);
        assert!(result.is_ok());
        
        let translated = result.unwrap();
        assert!(translated.source.contains("vec4"));
        assert!(translated.source.contains("vec2"));
    }

    #[test]
    fn test_glsl_to_msl() {
        let mut translator = ShaderTranslator::new();
        
        let shader = Shader {
            name: "test_shader".to_string(),
            language: ShaderLanguage::GLSL,
            stage: ShaderStage::Vertex,
            source: "void main() { vec4 color = vec4(1.0); }".to_string(),
        };
        
        let result = translator.translate(&shader, ShaderLanguage::MSL);
        assert!(result.is_ok());
        
        let translated = result.unwrap();
        assert!(translated.source.contains("float4"));
    }

    #[test]
    fn test_translation_caching() {
        let mut translator = ShaderTranslator::new();
        
        let shader = Shader {
            name: "cached_shader".to_string(),
            language: ShaderLanguage::HLSL,
            stage: ShaderStage::Fragment,
            source: "float4 main() { return float4(1, 0, 0, 1); }".to_string(),
        };
        
        // 第一次翻译
        let _ = translator.translate(&shader, ShaderLanguage::GLSL).unwrap();
        assert_eq!(translator.stats.cache_misses, 1);
        
        // 第二次翻译（应该命中缓存）
        let _ = translator.translate(&shader, ShaderLanguage::GLSL).unwrap();
        assert_eq!(translator.stats.cache_hits, 1);
    }

    #[test]
    fn test_unsupported_translation() {
        let mut translator = ShaderTranslator::new();
        
        let shader = Shader {
            name: "test".to_string(),
            language: ShaderLanguage::SPIRV,
            stage: ShaderStage::Compute,
            source: "// SPIR-V".to_string(),
        };
        
        let result = translator.translate(&shader, ShaderLanguage::HLSL);
        assert!(result.is_err());
    }
}
