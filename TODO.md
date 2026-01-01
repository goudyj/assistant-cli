# TODO - Assistant CLI

## Règles de développement

**À chaque itération / PR:**
- [x] `cargo build` - vérifier que le code compile
- [x] `cargo test` - tous les tests passent
- [x] `cargo clippy` - pas de warnings
- [x] Coverage suffisant des nouvelles fonctionnalités

**Structure des tests:**
- Tests unitaires dans chaque module (`#[cfg(test)] mod tests`)
- Tests d'intégration avec `wiremock` pour les appels GitHub API
- Pas de tests automatisés pour le TUI (tests manuels)

**Checklist avant de passer à la tâche suivante:**
1. La fonctionnalité marche (test manuel)
2. Tests unitaires ajoutés pour la nouvelle logique
3. `cargo test` passe
4. `cargo build --release` compile sans erreur

---

## 1. Clôturer une issue

**Objectif:** Fermer/rouvrir une issue directement depuis le TUI.

**Implémentation:**

### 1.1 API GitHub
- [x] Ajouter `GitHubConfig::close_issue(number)` - PATCH avec `state: "closed"`
- [x] Ajouter `GitHubConfig::reopen_issue(number)` - PATCH avec `state: "open"`

### 1.2 Interface utilisateur
- [x] Touche `x` dans la vue détail pour fermer l'issue
- [x] Afficher une confirmation: "Close issue #123? (y/n)"
- [x] Après fermeture: retour à la liste, issue retirée ou marquée comme fermée
- [x] Touche `X` (shift) pour rouvrir une issue fermée

### 1.3 Feedback visuel
- [x] Message de statut "Issue #123 closed" affiché temporairement
- [x] Différencier visuellement les issues fermées dans la liste (grisé, barré)

### 1.4 Fichiers à modifier
- [x] `src/github.rs`: Nouvelles méthodes
- [x] `src/tui.rs`: Handler de touche, confirmation, mise à jour de la liste

### 1.5 Tests à écrire
- [x] Test `close_issue()` avec wiremock (mock PATCH /issues/{number})
- [x] Test `reopen_issue()` avec wiremock
- [x] Test erreur 404 (issue inexistante)
- [x] Test erreur 403 (permissions insuffisantes)

---

## 2. Commande `/list` avec recherche intégrée

**Objectif:** Commande générique `/list` qui ouvre le TUI avec possibilité de filtrer/rechercher.

**Implémentation:**

### 2.1 Nouvelle commande `/list`
```
/list                       # Liste toutes les issues ouvertes
/list bug                   # Filtre par label "bug"
/list bug,customer          # Filtre par labels "bug" ET "customer"
/list auth                  # Recherche "auth" dans titre/body (si pas un label connu)
/list bug auth              # Label "bug" + recherche "auth"
/list "error handling"      # Recherche avec espaces (entre guillemets)
/list --state=closed        # Issues fermées
/list --state=closed auth   # Issues fermées contenant "auth"
```

### 2.2 Parser les arguments
- [x] Créer un struct `ListOptions { labels: Vec<String>, state: IssueState, search: Option<String> }`
- [x] Parser la syntaxe: labels séparés par virgule, options `--key=value`
- [x] Supporter `--state=open|closed|all`
- [x] Détecter si un argument est un label connu (défini dans config) ou un mot-clé de recherche
- [x] Supporter les guillemets pour recherche multi-mots: `"mon texte"`

### 2.3 API GitHub Search
- [x] Utiliser `octocrab` list issues avec state parameter (search via local filter)
- [x] Ajouter state parameter à `GitHubConfig::list_issues()` dans `github.rs`

### 2.4 Mode recherche dans le TUI (après ouverture)
- [x] Touche `/` pour entrer en mode recherche (comme vim)
- [x] Ajouter un champ `search_query: Option<String>` dans `IssueBrowser`
- [x] Afficher un champ de saisie en bas de l'écran
- [x] Filtrer les issues en temps réel pendant la saisie
- [x] `Escape` pour annuler, `Enter` pour valider et rester filtré

### 2.5 Logique de filtrage local (dans le TUI)
- [x] Créer `fn matches_query(issue: &IssueSummary, query: &str) -> bool`
- [x] Recherche case-insensitive sur: `title`, `labels`
- [x] Stocker les issues originales dans `all_issues: Vec<IssueSummary>`
- [x] `issues` devient la vue filtrée

### 2.6 Affichage
- [x] Afficher le query actif dans le titre: `" Issues (filtered: 'bug') "`
- [x] Touche `c` (clear) pour réinitialiser le filtre

### 2.7 Rétrocompatibilité
- [x] Garder les commandes dynamiques existantes (`list_commands` dans config) comme aliases
- [x] `/bugs` reste un alias pour `/list Bug`

### 2.8 Fichiers à modifier
- [x] `src/main.rs`: Nouveau handler pour `/list`, parser d'options
- [x] `src/github.rs`: Modifier `list_issues()` pour `State::All`
- [x] `src/list.rs`: Nouveau module pour ListOptions
- [x] `src/tui.rs`: Mode recherche, champ de saisie, logique de filtrage

### 2.9 Tests à écrire
- [x] Test parser `/list` sans arguments
- [x] Test parser `/list bug` (label seul)
- [x] Test parser `/list bug,feature` (multi-labels)
- [x] Test parser `/list auth` (recherche mot-clé)
- [x] Test parser `/list bug auth` (label + recherche)
- [x] Test parser `/list "error handling"` (guillemets)
- [x] Test parser `/list --state=closed`
- [x] Test `matches_query()` pour filtrage local (dans main.rs)

---

## 3. Infinite Scrolling pour la liste des issues

**Objectif:** Charger plus de 20 issues avec pagination automatique.

**Implémentation:**

### 3.1 Modifier `github.rs` pour supporter la pagination
- [x] Ajouter une méthode `list_issues_paginated()` qui retourne la page + info de pagination
- [x] Utiliser `octocrab::Page::next()` pour récupérer les pages suivantes
- [x] Retourne `(Vec<IssueSummary>, has_next_page: bool)`

### 3.2 Modifier `tui.rs` pour le chargement dynamique
- [x] Ajouter `has_next_page`, `current_page`, `is_loading` dans `IssueBrowser`
- [x] Détecter quand l'utilisateur atteint la fin de la liste (index >= issues.len() - 5)
- [x] Déclencher le chargement de la page suivante en async
- [x] Afficher un indicateur "[Loading...]" dans le titre
- [x] Fusionner les nouvelles issues dans `issues: Vec<IssueSummary>`

### 3.3 Fichiers à modifier
- [x] `src/github.rs`: Nouvelle méthode de pagination
- [x] `src/tui.rs`: Logique de détection et chargement
- [x] `src/main.rs`: Passer les infos de pagination au TUI

### 3.4 Tests à écrire
- [ ] Test pagination avec wiremock (mock Link header)
- [ ] Test `has_next_page` parsing
- [ ] Test fusion des pages (pas de doublons)

---

## 4. Attribution de personnes sur les issues

**Objectif:** Assigner des utilisateurs à une issue avec auto-complete.

**Implémentation:**

### 4.1 API GitHub pour les assignees
- [x] Ajouter `GitHubConfig::list_assignees()` - liste les collaborateurs du repo
- [x] Ajouter `GitHubConfig::assign_issue(number, assignees)`
- [x] Ajouter `GitHubConfig::unassign_issue(number, assignees)`
- [x] Cache la liste des assignees au démarrage du TUI

### 4.2 Interface utilisateur dans le TUI
- [x] Touche `a` dans la vue détail pour ouvrir le sélecteur d'assignees
- [x] Nouvelle vue `TuiView::AssignUser { issue, input, suggestions, selected }`
- [x] Filtrage en temps réel pendant la saisie (fuzzy matching)
- [x] Navigation avec flèches haut/bas dans les suggestions
- [x] `Enter` pour assigner, `Escape` pour annuler
- [x] Afficher les assignees actuels avec possibilité de les retirer

### 4.3 Structures de données
- [x] Ajouter `assignees: Vec<String>` dans `IssueSummary` et `IssueDetail`
- [x] Afficher les assignees dans la vue liste et détail

### 4.4 Fichiers à modifier
- [x] `src/github.rs`: Nouvelles méthodes API
- [x] `src/tui.rs`: Nouvelle vue, logique d'auto-complete

### 4.5 Tests à écrire
- [x] Test `list_assignees()` avec wiremock
- [x] Test `assign_issue()` avec wiremock
- [x] Test `unassign_issue()` avec wiremock
- [x] Test fuzzy matching pour auto-complete

---

## 5. Intégration Claude Code

**Objectif:** Dispatcher des issues à Claude Code pour les traiter automatiquement.

### 5.1 Configuration projet

Ajouter `local_path` dans `assistant.json` pour chaque projet:

```json
{
  "projects": {
    "assistant": {
      "owner": "jean",
      "repo": "assistant",
      "labels": ["bug", "feature"],
      "local_path": "/Users/jean/Documents/developpement/assistant"
    }
  }
}
```

### 5.2 Module `src/agents/mod.rs`

#### 5.2.1 Structures de données
```rust
pub struct AgentSession {
    pub id: String,                 // UUID unique
    pub issue_number: u64,
    pub issue_title: String,
    pub project: String,
    pub started_at: DateTime<Utc>,
    pub status: AgentStatus,
    pub pid: u32,
    pub log_file: PathBuf,          // ~/.cache/assistant/agents/<id>.log
    pub worktree_path: PathBuf,     // ~/.cache/assistant/worktrees/<project>-<issue>/
    pub branch_name: String,        // issue-123
    pub stats: AgentStats,
    pub pr_url: Option<String>,     // URL de la PR si créée
}

pub struct AgentStats {
    pub lines_output: usize,        // Lignes dans le log Claude
    pub lines_added: usize,         // git diff --numstat
    pub lines_deleted: usize,
    pub files_changed: usize,
}

pub enum AgentStatus {
    Running,
    Completed { exit_code: i32 },
    Failed { error: String },
}
```

#### 5.2.2 Gestion des sessions
- [ ] Fichier de sessions: `~/.cache/assistant/sessions.json`
- [ ] `fn list_sessions() -> Vec<AgentSession>`
- [ ] `fn get_session(id: &str) -> Option<AgentSession>`
- [ ] `fn update_session_status(id: &str, status: AgentStatus)`
- [ ] `fn cleanup_old_sessions(days: u32)` - supprime les vieilles sessions

### 5.3 Module `src/agents/claude.rs`

#### 5.3.1 Git Worktree pour isolation

Chaque issue est traitée dans un worktree isolé pour éviter les conflits:

```
~/.cache/assistant/worktrees/
├── <project>-<issue>/      # Un worktree par issue
│   └── ...                 # Copie complète du repo
```

**Flow de création:**
```bash
# 1. Créer une branche pour l'issue
git -C <local_path> branch issue-123 HEAD

# 2. Créer le worktree
git -C <local_path> worktree add ~/.cache/assistant/worktrees/project-123 issue-123
```

**Implémentation:**
- [ ] `fn create_worktree(local_path, issue_number) -> Result<PathBuf>`
- [ ] `fn remove_worktree(worktree_path) -> Result<()>`
- [ ] Ajouter `worktree_path: PathBuf` dans `AgentSession`
- [ ] Cleanup automatique des vieux worktrees (après X jours ou manuellement)

#### 5.3.2 Lancement de Claude Code
```rust
pub async fn dispatch_to_claude(
    issue: &IssueDetail,
    local_path: &Path,
) -> Result<AgentSession, AgentError>
```

- [ ] Créer le worktree pour l'issue
- [ ] Construire le prompt: `"Fix GitHub issue #{number}: {title}\n\n{body}"`
- [ ] Commande: `claude -p "<prompt>"`
- [ ] Working directory: le worktree (pas `local_path`)
- [ ] Rediriger stdout/stderr vers le fichier de log
- [ ] Lancer en background avec `std::process::Command`
- [ ] Retourner immédiatement avec `AgentSession`

#### 5.3.3 Monitoring du processus
- [ ] Thread/task de monitoring qui poll le processus
- [ ] Compter les lignes de sortie en temps réel
- [ ] **Git diff périodique** (toutes les 5s) pour stats de code:
  ```rust
  pub struct AgentStats {
      pub lines_output: usize,    // Lignes dans le log Claude
      pub lines_added: usize,     // git diff --numstat
      pub lines_deleted: usize,
      pub files_changed: usize,
  }
  ```
- [ ] Détecter la fin du processus
- [ ] Mettre à jour le statut dans `sessions.json`
- [ ] Envoyer notification macOS à la fin

#### 5.3.4 Actions post-completion
- [ ] Option "Create PR" → `gh pr create` depuis la branche du worktree
- [ ] Option "View diff" → afficher le diff dans le TUI
- [ ] Option "Cleanup" → supprimer le worktree et la branche
- [ ] Option "Open in editor" → ouvrir le worktree dans VS Code/editor

### 5.4 Notifications macOS

- [ ] Utiliser `osascript` ou crate `notify-rust`
- [ ] À la fin: "Claude Code finished issue #123"
- [ ] Clic sur notification → ouvre le log ou le TUI

```rust
fn send_macos_notification(title: &str, message: &str) {
    Command::new("osascript")
        .args(["-e", &format!(
            "display notification \"{}\" with title \"{}\"",
            message, title
        )])
        .spawn()
        .ok();
}
```

### 5.5 Interface TUI

#### 5.5.1 Dispatch depuis la vue détail
- [ ] Touche `d` pour dispatcher l'issue à Claude Code
- [ ] Confirmation: "Dispatch #123 to Claude Code? (y/n)"
- [ ] Message: "Agent started. View status with /agents"

#### 5.5.2 Multi-select dans la vue liste
- [ ] Touche `Space` pour sélectionner/désélectionner une issue
- [ ] Afficher un marqueur `[x]` devant les issues sélectionnées
- [ ] Touche `d` avec sélection → dispatcher toutes les issues sélectionnées
- [ ] Lancement en parallèle (une instance Claude Code par issue)

#### 5.5.3 Vue des sessions actives
- [ ] Commande `/agents` ou touche `A` dans le TUI
- [ ] Nouvelle vue `TuiView::AgentList`
- [ ] Afficher: issue #, titre, statut, lignes output, durée
- [ ] Navigation: `Enter` pour voir les logs, `k` pour kill un agent

```
┌──────────────────────────────────────────────────────────────────────┐
│ Agents (↑↓ navigate, Enter logs, p PR, o open, k kill, q back)       │
├──────────────────────────────────────────────────────────────────────┤
│ ▶ #123 Fix auth bug      Running    +47 -12   3 files   2m 30s       │
│   #124 Add dark mode     Completed  +203 -45  8 files   5m 12s  [PR] │
│   #125 Update deps       Failed     Error: timeout                   │
└──────────────────────────────────────────────────────────────────────┘
```

**Légende:**
- `+47 -12` : lignes ajoutées/supprimées (git diff)
- `3 files` : fichiers modifiés
- `[PR]` : PR créée depuis cette session

#### 5.5.4 Vue des logs
- [ ] Nouvelle vue `TuiView::AgentLogs { session_id }`
- [ ] Afficher le contenu du fichier de log
- [ ] Scroll avec j/k
- [ ] Refresh automatique si l'agent est encore en cours
- [ ] `q` pour revenir à la liste des agents

### 5.6 Commande CLI `/agents`

- [ ] `/agents` - Ouvre la vue des sessions actives
- [ ] `/agents list` - Liste les sessions en mode texte
- [ ] `/agents logs <id>` - Affiche les logs d'une session
- [ ] `/agents kill <id>` - Termine un agent en cours
- [ ] `/agents clean` - Supprime les vieilles sessions

### 5.7 Fichiers à créer/modifier

**Nouveaux fichiers:**
- [ ] `src/agents/mod.rs`: Module principal, structures
- [ ] `src/agents/claude.rs`: Intégration Claude Code
- [ ] `src/agents/session.rs`: Gestion des sessions

**Fichiers à modifier:**
- [ ] `src/config.rs`: Ajouter `local_path` à `ProjectConfig`
- [ ] `src/tui.rs`: Nouvelles vues (AgentList, AgentLogs), multi-select
- [ ] `src/main.rs`: Commande `/agents`
- [ ] `src/lib.rs`: Exposer le module `agents`

### 5.8 Dépendances à ajouter
- [ ] `uuid` pour les IDs de session
- [ ] `chrono` pour les timestamps

### 5.9 Tests à écrire
- [ ] Test `create_worktree()` - crée bien la branche et le worktree
- [ ] Test `remove_worktree()` - cleanup propre
- [ ] Test sérialisation/désérialisation `AgentSession` (JSON)
- [ ] Test `list_sessions()` - lecture du fichier sessions.json
- [ ] Test `update_session_status()`
- [ ] Test parsing `git diff --numstat` pour `AgentStats`
- [ ] Test `cleanup_old_sessions()` - supprime les vieilles sessions
- [ ] Test prompt generation (échappement des caractères spéciaux)

---

## 6. Intégrations futures (autres agents)

À implémenter après Claude Code, sur le même modèle:

### 6.1 Codex CLI
- Commande: `codex "<prompt>"`
- Même architecture que Claude Code

### 6.3 Architecture générique
Une fois les deux premiers agents implémentés, refactorer:
- Trait `Agent` avec `dispatch()`, `get_status()`, `kill()`
- Factory pour créer l'agent approprié
- Config optionnelle pour des agents custom

---

## Ordre d'implémentation

1. **Clôturer une issue** - Rapide, utile immédiatement
2. **Commande `/list` + recherche** - Améliore l'UX
3. **Infinite scrolling** - Nécessaire pour gros repos
4. **Attribution de personnes** - Plus complexe
5. **Intégration Claude Code** - Gros chantier, à découper:
   - 5a. Config `local_path` + structure sessions
   - 5b. Lancement basique de Claude Code
   - 5c. Monitoring + notifications
   - 5d. TUI multi-select + vue agents
   - 5e. Vue logs + commande `/agents`

---

## Notes techniques

**Dépendances à ajouter:**
- `fuzzy-matcher` pour l'auto-complete (tâche 4)
- `uuid` pour les sessions (tâche 5)
- `chrono` pour les timestamps (tâche 5)

**Fichiers de cache:**
```
~/.cache/assistant/
├── sessions.json       # Liste des sessions actives/terminées
├── agents/
│   └── <uuid>.log      # Logs de chaque session
└── worktrees/
    └── <project>-<issue>/  # Git worktrees isolés
```

**Commandes de test:**
```bash
cargo build              # Compilation
cargo test               # Tests unitaires + intégration
cargo test -- --nocapture  # Avec output
cargo clippy             # Linting
cargo test <module>::tests  # Tests d'un module spécifique
```
