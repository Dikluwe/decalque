//! Crystalline Lineage
//! @prompt 00_nucleo/prompts/pdf-error-policy.md
//! @layer L1
//! @updated 2026-08-13
//!
//! Taxonomia de erros de leitura de PDF. Vive em `01_core` porque é domínio:
//! `02_shell` apresenta-os ao utilizador e `03_infra` apenas converte erros do
//! lopdf para este tipo — defini-lo em `03_infra` criaria duas taxonomias.

use std::fmt;

/// Erro de leitura de um PDF.
///
/// Cada variante carrega o que `02_shell` precisa para compor a mensagem sem
/// ter de interpretar a variante: o `Display` já produz texto legível.
#[derive(Debug, Clone, PartialEq)]
pub enum PdfError {
    /// Ficheiro não encontrado, sem permissão, ou falha de leitura.
    Io { message: String },
    /// Estrutura inválida: xref, stream indecodificável, `MediaBox` ausente
    /// mesmo por herança da árvore de páginas.
    Parse { message: String },
    /// PDF criptografado. A v1 não aceita senha (ADR 0002); o caminho
    /// `Document::load_with_password` fica registado para versão futura.
    Encrypted,
    /// Senha inválida.
    ///
    /// Existe na taxonomia desde já porque o ADR 0002 prevê suporte a senha
    /// numa versão futura, mas **nenhum caminho da v1 a produz** — sem
    /// aceitação de senha, um PDF criptografado pára sempre em `Encrypted`.
    InvalidPassword,
    /// Recurso PDF fora do escopo suportado (filtro não suportado, etc.).
    Unsupported { message: String },
    /// `page_index` fora do intervalo de páginas do documento.
    PageNotFound { page_index: usize },
}

impl fmt::Display for PdfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PdfError::Io { message } => {
                write!(f, "erro de leitura do ficheiro: {message}")
            }
            PdfError::Parse { message } => {
                write!(f, "estrutura de PDF inválida: {message}")
            }
            PdfError::Encrypted => write!(f, "PDF criptografado: esta versão não aceita senha"),
            PdfError::InvalidPassword => write!(f, "senha inválida para o PDF criptografado"),
            PdfError::Unsupported { message } => {
                write!(f, "recurso de PDF não suportado: {message}")
            }
            PdfError::PageNotFound { page_index } => {
                write!(f, "página {page_index} não existe no documento")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn todas_as_variantes() -> Vec<PdfError> {
        vec![
            PdfError::Io {
                message: "ficheiro não encontrado".to_string(),
            },
            PdfError::Parse {
                message: "xref inválido".to_string(),
            },
            PdfError::Encrypted,
            PdfError::InvalidPassword,
            PdfError::Unsupported {
                message: "filtro JPXDecode".to_string(),
            },
            PdfError::PageNotFound { page_index: 7 },
        ]
    }

    #[test]
    fn cada_variante_tem_display_legivel_e_distinto() {
        let mensagens: Vec<String> = todas_as_variantes().iter().map(|e| e.to_string()).collect();
        for m in &mensagens {
            assert!(!m.is_empty(), "mensagem não pode ser vazia");
        }
        // O shell distingue as variantes pelo texto, sem `match`.
        for (i, a) in mensagens.iter().enumerate() {
            for b in mensagens.iter().skip(i + 1) {
                assert_ne!(a, b, "duas variantes produziram a mesma mensagem");
            }
        }
    }

    #[test]
    fn variantes_com_mensagem_incluem_a_mensagem_no_display() {
        let e = PdfError::Io {
            message: "permissão negada".to_string(),
        };
        assert!(e.to_string().contains("permissão negada"));
        let e = PdfError::Parse {
            message: "MediaBox ausente".to_string(),
        };
        assert!(e.to_string().contains("MediaBox ausente"));
        let e = PdfError::Unsupported {
            message: "filtro JPXDecode".to_string(),
        };
        assert!(e.to_string().contains("filtro JPXDecode"));
    }

    #[test]
    fn page_not_found_expoe_o_indice_pedido() {
        let e = PdfError::PageNotFound { page_index: 7 };
        // Payload acessível: o shell pode dizer "página 7 não existe".
        match &e {
            PdfError::PageNotFound { page_index } => assert_eq!(*page_index, 7),
            outro => panic!("variante inesperada: {outro:?}"),
        }
        assert!(e.to_string().contains('7'));
    }

    #[test]
    fn encrypted_nao_menciona_senha_invalida() {
        // Na v1 são situações distintas: `Encrypted` é "não aceito senha",
        // `InvalidPassword` é "a senha dada está errada" (sem produtor na v1).
        let encrypted = PdfError::Encrypted.to_string();
        let invalida = PdfError::InvalidPassword.to_string();
        assert_ne!(encrypted, invalida);
    }
}
