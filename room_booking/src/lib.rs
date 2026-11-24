//! Réservation de salles — bibliothèque.
//!
//! Couches (clean architecture), de l'intérieur vers l'extérieur :
//! - `domain`      : entités et règles métier (R1–R6, expiration, check-in…), sans dépendance externe ;
//! - `application` : use cases et ports (traits) que l'infrastructure implémente ;
//! - `infrastructure` : implémentations concrètes des ports (horloge, notifications, dépôts).
//!
//! `main.rs` ne contient que le point d'entrée et le branchement des adaptateurs.

pub mod application;
pub mod domain;
pub mod infrastructure;
