use serde::Deserialize;
use serde::Serialize;

/// Supported programming languages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Language {
    Rust,
    TypeScript,
    JavaScript,
    Python,
    CSharp,
    CSharpUnity,
}

impl Language {
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "rs" => Some(Self::Rust),
            "ts" | "tsx" => Some(Self::TypeScript),
            "js" | "jsx" => Some(Self::JavaScript),
            "py" | "pyw" => Some(Self::Python),
            "cs" => Some(Self::CSharp),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Rust => "Rust",
            Self::TypeScript => "TypeScript",
            Self::JavaScript => "JavaScript",
            Self::Python => "Python",
            Self::CSharp => "C#",
            Self::CSharpUnity => "C# (Unity)",
        }
    }

    pub fn extensions(&self) -> &'static [&'static str] {
        match self {
            Self::Rust => &["rs"],
            Self::TypeScript => &["ts", "tsx"],
            Self::JavaScript => &["js", "jsx"],
            Self::Python => &["py", "pyw"],
            Self::CSharp | Self::CSharpUnity => &["cs"],
        }
    }
}

/// Code review severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ReviewSeverity {
    Critical = 4,
    High = 3,
    Medium = 2,
    Low = 1,
    Info = 0,
}

impl ReviewSeverity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Critical => "Critical",
            Self::High => "High",
            Self::Medium => "Medium",
            Self::Low => "Low",
            Self::Info => "Info",
        }
    }
}

/// Code review result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewResult {
    pub file_path: String,
    pub language: Language,
    pub issues: Vec<ReviewIssue>,
    pub summary: ReviewSummary,
}

/// Individual review issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewIssue {
    pub line: usize,
    pub column: Option<usize>,
    pub severity: ReviewSeverity,
    pub category: String,
    pub message: String,
    pub suggestion: Option<String>,
    pub rule: Option<String>,
}

/// Review summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewSummary {
    pub total_issues: usize,
    pub critical_count: usize,
    pub high_count: usize,
    pub medium_count: usize,
    pub low_count: usize,
    pub info_count: usize,
}

impl ReviewSummary {
    pub fn from_issues(issues: &[ReviewIssue]) -> Self {
        let mut summary = Self {
            total_issues: issues.len(),
            critical_count: 0,
            high_count: 0,
            medium_count: 0,
            low_count: 0,
            info_count: 0,
        };

        for issue in issues {
            match issue.severity {
                ReviewSeverity::Critical => summary.critical_count += 1,
                ReviewSeverity::High => summary.high_count += 1,
                ReviewSeverity::Medium => summary.medium_count += 1,
                ReviewSeverity::Low => summary.low_count += 1,
                ReviewSeverity::Info => summary.info_count += 1,
            }
        }

        summary
    }
}

/// Review locale for internationalization (ISO 639-1 codes)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReviewLocale {
    /// Japanese (日本語)
    Japanese,
    /// English
    English,
    /// Chinese (中文)
    Chinese,
    /// Korean (한국어)
    Korean,
    /// French (Français)
    French,
    /// German (Deutsch)
    German,
    /// Spanish (Español)
    Spanish,
    /// Portuguese (Português)
    Portuguese,
}

impl ReviewLocale {
    /// Parse from language code (e.g., "ja", "en", "zh")
    pub fn from_code(code: &str) -> Option<Self> {
        match code.to_lowercase().as_str() {
            "ja" | "jp" | "japanese" => Some(Self::Japanese),
            "en" | "english" => Some(Self::English),
            "zh" | "cn" | "chinese" => Some(Self::Chinese),
            "ko" | "kr" | "korean" => Some(Self::Korean),
            "fr" | "french" => Some(Self::French),
            "de" | "german" => Some(Self::German),
            "es" | "spanish" => Some(Self::Spanish),
            "pt" | "portuguese" => Some(Self::Portuguese),
            _ => None,
        }
    }

    /// Get ISO 639-1 language code
    pub fn code(&self) -> &'static str {
        match self {
            Self::Japanese => "ja",
            Self::English => "en",
            Self::Chinese => "zh",
            Self::Korean => "ko",
            Self::French => "fr",
            Self::German => "de",
            Self::Spanish => "es",
            Self::Portuguese => "pt",
        }
    }

    /// Get localized system prompt for code review
    pub fn system_prompt(&self) -> &'static str {
        match self {
            Self::Japanese => r#"あなたは経験豊富なコードレビュアーです。
以下のコードをレビューして、改善点を指摘してください。

レビュー観点：
1. セキュリティの問題
2. パフォーマンスの問題
3. ベストプラクティスからの逸脱
4. エラーハンドリングの不備
5. コードの可読性

フォーマット：
- 問題点は具体的に指摘してください
- 改善提案を必ず含めてください
- 良い点も忘れずに評価してください"#,
            Self::English => r#"You are an experienced code reviewer.
Please review the following code and point out improvements.

Review aspects:
1. Security issues
2. Performance issues
3. Deviations from best practices
4. Error handling deficiencies
5. Code readability

Format:
- Point out issues specifically
- Always include improvement suggestions
- Don't forget to appreciate good points"#,
            Self::Chinese => r#"您是一位经验丰富的代码审查员。
请审查以下代码并指出改进之处。

审查方面：
1. 安全问题
2. 性能问题
3. 偏离最佳实践
4. 错误处理不足
5. 代码可读性

格式：
- 具体指出问题
- 始终包括改进建议
- 不要忘记赞赏优点"#,
            Self::Korean => r#"당신은 경험이 풍부한 코드 리뷰어입니다.
다음 코드를 검토하고 개선점을 지적해 주세요.

검토 항목:
1. 보안 문제
2. 성능 문제
3. 모범 사례 위반
4. 오류 처리 부족
5. 코드 가독성

형식:
- 문제를 구체적으로 지적하세요
- 항상 개선 제안을 포함하세요
- 좋은 점도 평가하는 것을 잊지 마세요"#,
            Self::French => r#"Vous êtes un réviseur de code expérimenté.
Veuillez examiner le code suivant et signaler les améliorations.

Aspects de révision:
1. Problèmes de sécurité
2. Problèmes de performance
3. Déviations des meilleures pratiques
4. Déficiences dans la gestion des erreurs
5. Lisibilité du code

Format:
- Signalez les problèmes de manière spécifique
- Incluez toujours des suggestions d'amélioration
- N'oubliez pas d'apprécier les bons points"#,
            Self::German => r#"Sie sind ein erfahrener Code-Reviewer.
Bitte überprüfen Sie den folgenden Code und weisen Sie auf Verbesserungen hin.

Überprüfungsaspekte:
1. Sicherheitsprobleme
2. Leistungsprobleme
3. Abweichungen von Best Practices
4. Mängel bei der Fehlerbehandlung
5. Code-Lesbarkeit

Format:
- Weisen Sie spezifisch auf Probleme hin
- Fügen Sie immer Verbesserungsvorschläge hinzu
- Vergessen Sie nicht, gute Punkte zu würdigen"#,
            Self::Spanish => r#"Eres un revisor de código experimentado.
Por favor, revisa el siguiente código y señala mejoras.

Aspectos de revisión:
1. Problemas de seguridad
2. Problemas de rendimiento
3. Desviaciones de las mejores prácticas
4. Deficiencias en el manejo de errores
5. Legibilidad del código

Formato:
- Señala los problemas específicamente
- Incluye siempre sugerencias de mejora
- No olvides apreciar los puntos buenos"#,
            Self::Portuguese => r#"Você é um revisor de código experiente.
Por favor, revise o código a seguir e aponte melhorias.

Aspectos de revisão:
1. Problemas de segurança
2. Problemas de desempenho
3. Desvios das melhores práticas
4. Deficiências no tratamento de erros
5. Legibilidade do código

Formato:
- Aponte os problemas especificamente
- Sempre inclua sugestões de melhoria
- Não esqueça de apreciar os pontos bons"#,
        }
    }

    /// Get localized header for review results
    pub fn review_header(&self) -> &'static str {
        match self {
            Self::Japanese => "🔍 コードレビュー結果",
            Self::English => "🔍 Code Review Results",
            Self::Chinese => "🔍 代码审查结果",
            Self::Korean => "🔍 코드 리뷰 결과",
            Self::French => "🔍 Résultats de la révision du code",
            Self::German => "🔍 Code-Review-Ergebnisse",
            Self::Spanish => "🔍 Resultados de la revisión del código",
            Self::Portuguese => "🔍 Resultados da revisão do código",
        }
    }
}

impl Default for ReviewLocale {
    fn default() -> Self {
        Self::English
    }
}
