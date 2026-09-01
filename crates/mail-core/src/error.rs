/// Erreurs du domaine.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("adresse email invalide : {0:?}")]
    InvalidEmailAddress(String),

    #[error("stockage : {0}")]
    Storage(#[from] rusqlite::Error),

    /// Erreur remontée par une implémentation de [`crate::MailServer`]
    /// (réseau, protocole, authentification…).
    #[error("serveur : {0}")]
    Server(String),

    /// Refus EXPLICITE du serveur (NO/BAD : dossier disparu, `[CANNOT]`,
    /// `[TRYCREATE]`) — réessayer ne changera rien. Tout le reste
    /// (`Server`) est réputé transitoire : réseau, bridage, timeout.
    /// C'est la distinction que la boîte d'envoi a depuis l'ADR 0003
    /// (`SendError::{Transient, Permanent}`) et que le journal d'actions
    /// n'avait pas (audit 2026-09-01 S1-7, PLAN-AUDIT-V1 E3).
    #[error("refus du serveur : {0}")]
    Refus(String),

    /// Donnée locale inattendue (base modifiée hors de l'application).
    #[error("donnée locale invalide : {0}")]
    Corrupt(String),

    /// Un vocabulaire fermé du Mode organisé (destination du routage,
    /// règle du Non) a reçu un mot hors table — refusé avant toute
    /// écriture (PLAN-MODE-ORGANISE E1).
    #[error("routage invalide : {0}")]
    InvalidRouting(String),

    /// La pièce ferait déborder le plafond d'un message (PJ-D3) : rien
    /// n'est joint — le refus se joue au geste, jamais à l'envoi. Les
    /// tailles permettent à la surface de dire la place restante.
    #[error(
        "pièce trop lourde : {name:?} ({size} octets) dépasse la place restante ({remaining} octets)"
    )]
    AttachmentOverBudget {
        name: String,
        size: u64,
        remaining: u64,
    },

    /// L'utilisateur a annulé la migration d'une base héritée pendant la
    /// passe d'adoption. Tout a été défait (`ROLLBACK`), `user_version`
    /// est inchangé : la passe entière se rejouera au prochain lancement.
    #[error("migration interrompue")]
    Interrupted,
}
