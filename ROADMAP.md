# RustFrame — roadmap vers une adoption réelle

> Audit du 8 septembre 2026, checkout `bae1321` (`0.1.0-rc.2`).
> Exécution en cours. Les publications restent soumises à autorisation ; la vidéo sera réalisée en dernier, après les autres travaux locaux, conformément à la demande du 8 septembre.
> Les cases ouvertes sont du travail futur. La présence de code ou d’un workflow ne prouve pas une validation native ni une publication réussie.

## Suivi d’exécution

Les preuves et limites sont consignées dans [le journal de validation](docs/validation/roadmap-execution.md). Le CLI macOS ARM public RC2 est téléchargé, son checksum vérifié et `doctor` passe. Le quickstart complet reste bloqué par les publications npm/runtime. Le smoke public local a été corrigé pour refuser ce faux succès ; sa validation distante reste à effectuer. La vidéo est déplacée en dernière étape d’exécution à la demande de l’utilisateur.

### Validations locales acquises

- [x] Tests Rust du workspace : 164 réussites après corrections de la restauration et de la resélection de dossier ; Clippy et formatage passent.
- [x] API frontend : compilation, tests et tarball npm local construits.
- [x] Sept templates du dépôt construits et validés.
- [x] Cinq starters autonomes créés et validés avec le tarball local ; reçus chronométrés conservés. La validation publique/native reste distincte.
- [x] Tests navigateur après corrections : 30 réussites, deux exclusions desktop/mobile prévues.
- [x] Premier lancement natif inspecté ; titre surdimensionné corrigé et rendu natif revérifié.

## 1. Diagnostic : le potentiel existe, mais le premier succès est bloqué

RustFrame possède déjà une proposition intéressante : construire des outils desktop centrés sur les données locales avec un frontend habituel, SQLite et des accès aux fichiers explicitement autorisés, sans écrire et maintenir soi-même le backend Rust standard.

**Ce qui manque le plus aujourd’hui : une installation publique complète, une preuve visuelle immédiate et des utilisateurs externes qui réussissent à construire quelque chose.** Ajouter vingt fonctionnalités avant de résoudre ces trois points disperserait le projet.

Les stars sont un résultat possible de cette progression, jamais une garantie. Le bon objectif est de rendre le projet facile à comprendre, à essayer, à recommander et à contribuer.

### État vérifié pendant cet audit

| Observation | Preuve | Conséquence |
| --- | --- | --- |
| 27 stars, 1 fork ; Discussions activées | API GitHub consultée le 8 septembre | Une petite audience existe ; aucun diagnostic de conversion n’est possible sans données de visites et d’activation. |
| Deux prereleases, dernière `v0.1.0-rc.2` | `gh release list` | Il faut terminer le parcours de distribution, pas inventer un système de release. |
| RC2 contient 14 assets, centrés sur le CLI ; aucun installateur Research Desk dans cette release | `gh release view v0.1.0-rc.2 --json assets` | Le visiteur ne dispose pas ici du téléchargement évident de l’application montrée. |
| Les archives CLI RC2 affichent 5 téléchargements ARM macOS, 0 Intel macOS, 1 Windows et 1 Linux | Compteurs GitHub au moment de l’audit | Signal très limité ; ces téléchargements peuvent inclure CI et mainteneur, pas uniquement de nouveaux utilisateurs. |
| `npm view rustframe-api version --json` retourne `E404` | Registre npm consulté pendant l’audit | Le quickstart public est bloqué à l’installation du frontend. Priorité absolue. |
| README avertit de ce blocage, annonce GitHub RC2 et crates.io RC1 | [README](README.md) | Décalage de versions à résoudre ; versions crates.io rapportées par le README, non revérifiées dans le registre pendant cet audit. |
| Le smoke public accepte un chemin CLI seul si npm manque pour une prerelease | [Workflow](.github/workflows/public-artifact-smoke.yml) | Un workflow vert ne garantit pas le succès du quickstart complet. |
| Pas de référence vidéo MP4/WebM dans le README ou la homepage inspectés | [README](README.md), [homepage](site/index.html) | Le visiteur doit lire beaucoup avant de voir le résultat. |
| Le benchmark publié est un reçu macOS, version RC1, avec exclusions explicites | [Données](site/assets/data/research-desk-benchmark.json) | Ne pas présenter ces nombres comme une performance native complète de RC2 ou une comparaison universelle. |

### Déjà présent : conserver et rendre visible

L’ancienne roadmap mélangeait plusieurs chantiers accomplis et futurs. Cette version remplace cet état historique par des priorités actuelles.

- **Projet autonome et starters** : CLI, génération de runner, codegen, manifeste et starters frontend existent dans [le CLI](crates/rustframe-cli/src).
- **Données et workflow** : base locale, recherche, transactions et exports ont déjà une implémentation ; voir [database.rs](crates/rustframe/src/database.rs) et [Research Desk](apps/research-desk/app.js).
- **Permissions explicables** : [capabilities.rs](crates/rustframe-cli/src/capabilities.rs), [local_first.rs](crates/rustframe-cli/src/local_first.rs) et [release_verification.rs](crates/rustframe-cli/src/release_verification.rs) existent. Ne pas les remettre dans une liste de fonctions à créer.
- **Distribution** : workflows de packages natifs, publication des registres, vérification publique et release Research Desk existent déjà dans [.github/workflows](.github/workflows).
- **Communauté** : Discussions et des issues contributrices existent déjà. Améliorer leur utilisation au lieu de refaire leur configuration.
- **Présentation** : site, documentation, case study et benchmarks existent. Priorité à leur clarté et à leurs preuves, sans lancer une refonte totale par défaut.

Cet audit est une lecture ciblée du code et des surfaces publiques ; il ne constitue pas une nouvelle exécution de la suite de tests, une revue de sécurité complète ou une QA native.

## 2. La promesse qui mérite d’être partagée

Proposition de message principal, à tester auprès de développeurs externes :

> **Turn a folder of documents into a local desktop tool. TypeScript, SQLite, native file access — without writing the Rust backend.**

Sous-texte : un projet Vite normal, un schéma de données, des permissions explicites, une application installable. Le Rust reste nécessaire à la compilation native actuelle ; « sans écrire Rust » ne veut pas dire « sans toolchain Rust ».

La cible initiale : développeurs frontend construisant des outils de recherche, catalogues, files de revue documentaire et petits outils internes hors ligne. Research Desk doit rendre cette promesse tangible.

Tauri prend déjà en charge les frontends web, les plateformes desktop/mobile et un large ensemble de plugins. La différenciation proposée de RustFrame est donc **le workflow local intégré et le code applicatif économisé**, à démontrer sur une tâche équivalente. Elle n’est pas une supériorité de sécurité ou de performance présumée. [Documentation officielle Tauri](https://v2.tauri.app/start/).

## 3. Ordre d’exécution

Estimations indicatives pour un mainteneur, hors attente de comptes, certificats et retours externes. Les critères de sortie priment sur les dates.

| Priorité | Résultat | Effort estimé | Dépendance |
| --- | --- | --- | --- |
| P0 | Un inconnu installe et lance son projet depuis les artefacts publics | 2–4 jours + accès registres | Aucune |
| P1 | Research Desk prouve un workflow utile et reproductible | 3–5 jours | P0 pour la preuve publique ; QA locale possible avant |
| P2 | Démo réelle et README compréhensible en 30 secondes | 2–3 jours | Parcours P1 validé |
| P3 | Release coordonnée et téléchargements vérifiés | 3–5 jours + signature | P0, P1 ; P2 avant campagne |
| P4 | Première vague de builders externes et corrections | 2 semaines d’observation | P0–P3 |
| P5 | Investissements produit fondés sur les blocages observés | Itératif | Retours P4 |

## 4. P0 — Débloquer le premier projet public

Responsable : mainteneur release. C’est le chantier qui conditionne le retour sur tous les autres.

- [ ] Choisir une version coordonnée disponible pour runtime, CLI et API ; vérifier les dépendances exactes générées avant publication.
- [ ] Publier `rustframe-api` avec un accès npm autorisé ; utiliser le workflow existant et des secrets configurés hors du dépôt et du chat.
- [ ] Vérifier l’état crates.io en direct puis publier les versions nécessaires dans l’ordre de [la checklist](docs/release-checklist.md).
- [ ] Installer le CLI téléchargé, créer un projet dans un dossier extérieur au checkout et installer ses dépendances uniquement depuis les registres publics.
- [ ] Tester `doctor`, `new`, `validate`, `dev`, `build`, `package --verify` puis le démarrage de l’application produite sur macOS, Windows et Linux.
- [ ] Vérifier les starters vanilla TS et React en priorité, puis la matrice des autres starters annoncés.
- [ ] Afficher séparément « CLI téléchargeable » et « quickstart complet validé ». Pour la candidate promue, exiger l’exécution du chemin npm ; aucun fallback CLI seul ne peut satisfaire ce gate.
- [ ] Synchroniser README, docs, version du site et commandes avec les artefacts effectivement publiés.
- [ ] Mesurer installation CLI, prérequis, première compilation et premier affichage séparément ; documenter les dépendances Linux et les coûts de compilation.

**Sortie :** reçus des trois OS pour la même version, sans override local, sans paquet copié depuis le checkout et sans étape essentielle ignorée. Un développeur extérieur reproduit le quickstart avec les instructions publiques.

## 5. P1 — Faire de Research Desk une raison d’essayer

Responsable : mainteneur produit. Le scénario central : « je choisis mes documents, je trouve une information, je la classe et je récupère mes données ».

- [x] Premier lancement sur profil vide vérifié nativement : zéro document/accès au départ, choix du corpus dédié, indexation de deux fichiers et recherche. Lanceur de test avec seul `data_dir` isolé (et nom de binaire distinct), mêmes assets et runtime ; aucun chemin personnel dans les écrans vérifiés.
- [x] Parcourir sélection de dossier, consentement, indexation, recherche, lecture, annotation et export réel — validé nativement sur macOS ARM ; voir les reçus dans le journal.
- [x] Vérifier la synchronisation entre fenêtres et la persistance des annotations après fermeture et réouverture — note persistée après relance puis modification lecteur→fenêtre principale vérifiée sur macOS ARM.
- [x] Tester fichier modifié, renommé, supprimé, illisible, dossier vide et accès révoqué ; corriger les états trompeurs avant le tournage — parcours natif macOS et reçus consignés.
- [x] Vérifier hors ligne le binaire packagé, avec lecture et écriture réelles : processus natif testé sous refus des connexions externes, loopback autorisé ; portée WebKit/XPC explicitée dans le journal.
- [x] Clavier/focus exercés nativement et dans les tests navigateur ; Axe desktop/mobile et rendu Retina inspectés. Les défauts de saisie et de recherche observés sont corrigés. Ce n’est pas un audit complet d’accessibilité de tous les OS.
- [x] Mesures natives documentées : indexation 500 fichiers, recherche rendue dans l’arbre d’accessibilité et lancement jusqu’au champ activé. Échantillons uniques avec cache chaud et coût d’automatisation explicité ; voir [les mesures](docs/validation/native-benchmark.md).
- [x] Examiner la mémoire et documenter la portée : RSS natif mesuré, attribution des services WebKit externes non disponible ; aucun chiffre de mémoire totale revendiqué. Voir [la mesure native](docs/validation/native-benchmark.md).

**Sortie :** un parcours natif reproductible sur le build identifié, données exportées vérifiées et défauts bloquants corrigés. La vidéo peut alors montrer un résultat réellement obtenu.

## 6. P2 — Produire la démo vidéo réelle, puis simplifier l’entrée

Responsable : mainteneur produit et média. **Montage local terminé et vérifié, après les validations locales.** Captures de la fenêtre native packagée avec le skill `ffmpeg-video-editor` ; publication et tutoriel depuis les registres restent conditionnés aux accès externes.

### Film principal : 45–60 secondes

| Temps indicatif | Action réellement filmée | Ce que le visiteur comprend |
| --- | --- | --- |
| 0–5 s | Application native ouverte, recherche et résultat lisible | Le résultat avant l’explication |
| 5–13 s | Sélection d’un dossier de démo via le dialogue natif | Les documents restent dans un dossier choisi |
| 13–24 s | Indexation, recherche d’un terme présent, ouverture du document | Les fichiers alimentent un outil utile |
| 24–35 s | Annotation ou classement et synchronisation d’une fenêtre de lecture | Ce n’est pas une simple WebView décorative |
| 35–45 s | Export puis ouverture du fichier obtenu | Les données sont récupérables |
| 45–60 s | Court extrait du projet TypeScript et du schéma ; version et liens | Le lien entre l’application et le framework |

Le montage peut raccourcir les attentes, avec indication des accélérations. Ne pas laisser croire qu’une compilation coupée au montage a duré deux secondes. Une démonstration « hors ligne » doit provenir du test natif correspondant.

### Capture et montage FFmpeg

- [x] Vraie fenêtre native, bridge SQLite/fichiers et dialogues macOS enregistrés depuis le package `13926ad`.
- [x] Version, commit, OS, corpus, procédure, probes et prises brutes conservés hors de Git ; [provenance](docs/native-demo.md).
- [x] Le film et la nouvelle capture README proviennent du package natif ; aucune capture du bridge simulé utilisée.
- [x] Prises analysées avec `ffprobe`, timestamps normalisés et durées de sortie vérifiées.
- [x] Montage 16:9 à vitesse réelle, textes courts, coupes documentées et masquage explicite des libellés personnels.
- [x] Master, MP4 web H.264/yuv420p fast-start, miniature réelle et sous-titres VTT/SRT produits ; reçus dans le dossier de livraison.
- [x] Timeline complète inspectée image par image à une seconde d’intervalle ; détails de cadrage/masquage vérifiés. Lecture intégrale dans le navigateur arrivée à `ended=true`, contrôles présents.

Exemple d’encodage futur, à adapter après inspection de la prise ; il ne constitue pas une commande déjà exécutée :

```bash
ffprobe -v error -show_streams -show_format -of json capture.mov
ffmpeg -i capture.mov \
  -vf "scale=1920:1080:force_original_aspect_ratio=decrease,pad=1920:1080:(ow-iw)/2:(oh-ih)/2,fps=30" \
  -c:v libx264 -crf 23 -preset slow -pix_fmt yuv420p \
  -an -movflags +faststart rustframe-demo.mp4
ffprobe -v error -show_streams -show_format -of json rustframe-demo.mp4
```

Cet exemple produit une version silencieuse. Pour un tutoriel commenté, conserver la voix en AAC et contrôler son niveau avec le skill.

### Deux formats, deux usages

- [x] Film court local : 46,47 s, 1920 × 1080, 30 fps, environ 1,51 Mo. Hébergement public encore ouvert.
- [ ] Tutoriel de 3–5 minutes : installation publique, projet autonome, changement de schéma, fenêtre native, packaging. Fournir les durées réelles de build et les commandes complètes.
- [ ] Héberger les fichiers hors de l’historique source ; vérifier le lecteur réellement rendu sur GitHub, avec lecture complète et contrôles. Un GIF ne remplace pas la vidéo.
- [x] Lecteur, poster, contrôles, sous-titres, transcription et `preload="metadata"` préparés et vérifiés dans le site livré localement. Le manifeste source reste sans URL vidéo publique avant autorisation.

### README : réduire le temps avant compréhension

- [ ] En haut : promesse concrète, vidéo réelle, lien d’installation fonctionnel et téléchargement de Research Desk.
- [x] README : schéma minimal et API TypeScript cohérents, avec limites du parcours public visibles.
- [x] Statut RC, gates registres/signatures et portée native macOS visibles ; catalogue CLI détaillé renvoyé aux docs.
- [x] Aligner aperçu social, première section du site, release et README sur le même résultat : même promesse, même candidate `0.1.0-rc.2`, et même preuve native liée.
- [ ] Faire lire cette entrée à cinq développeurs inconnus du projet : au moins quatre doivent pouvoir expliquer la cible et trouver le premier essai sans aide.

**Sortie :** vidéo vérifiée sur le lecteur final, démonstration reproductible et premier écran compris. Aucun tournage d’une installation publique qui dépend encore d’un contournement privé.

## 7. P3 — Une release qui donne envie d’installer

Responsable : mainteneur release. Réutiliser les pipelines existants ; leur existence n’est pas une preuve de signature ou de publication.

- [ ] Publier d’abord une candidate coordonnée si les garanties stable ne sont pas encore réunies ; ne pas annoncer une version majeure pour le seul effet marketing.
- [ ] Distinguer clairement les artefacts du framework des installateurs de Research Desk, avec versions et liens réciproques.
- [ ] Pour chaque hôte annoncé : télécharger, vérifier les checksums/provenance, installer, lancer, exercer le parcours P1 et désinstaller sur l’OS natif.
- [ ] Pour Research Desk distribué comme produit de confiance : signature/notarisation macOS et signature Windows ; vérifier après téléchargement avec le workflow dédié.
- [x] Preview locale explicitement non signée ; vérification d’intégrité réussie avec `trusted:false`. Le gate de release de confiance reste ouvert.
- [ ] Tester mise à niveau depuis la candidate précédente, conservation des données, sauvegarde/restauration et stratégie de récupération.
- [ ] Joindre checksums, SBOM, provenance, OS testés et limitations réellement observées.
- [x] Notes de préparation écrites : [release locale](docs/launch/release-notes-draft.md), installation, changements, migration et limites ; liens publics différés jusqu’à publication.
- [ ] Vérifier les URLs finales et les assets après publication, puis mettre à jour les surfaces publiques.

**Gate stable :** P0 complet, QA native P1, migrations vérifiées, artefacts publics exacts testés, aucune étape critique ignorée et retours externes traités. « Schéma v1 » et « version stable du produit » doivent rester deux notions distinctes.

Les releases GitHub associent un tag, des notes et des fichiers distribuables : une archive de sources automatique ne suffit pas à offrir une application installable. [Documentation officielle GitHub](https://docs.github.com/en/repositories/releasing-projects-on-github/about-releases).

## 8. P4 — Transformer les essais en adoption et contributions

Responsable : mainteneur communauté. Commencer après P0 et une candidate vérifiée ; le stable peut attendre les retours.

- [ ] Recruter 5–10 développeurs frontend intéressés par un vrai outil local ; leur demander de construire un petit workflow, pas simplement de donner une star.
- [ ] Noter consentement, OS, blocage, temps jusqu’au premier succès et besoin réel, sans télémétrie imposée dans le runtime.
- [ ] Corriger les trois obstacles les plus fréquents avant une nouvelle vague de communication.
- [ ] Réutiliser Discussions pour les questions et publier des réponses reproductibles.
- [ ] Maintenir 5–8 issues réellement prêtes, avec critères de sortie et commande de vérification ; ne pas recréer les tickets existants.
- [ ] S’appuyer sur [#7](https://github.com/OthmaneBlial/rustframe/issues/7) pour le temps jusqu’à la première fenêtre, [#11](https://github.com/OthmaneBlial/rustframe/issues/11) pour les upgrades et [#12](https://github.com/OthmaneBlial/rustframe/issues/12) pour l’accessibilité des templates.
- [ ] Obtenir deux applications externes reproductibles avant de présenter le showcase comme un écosystème actif.
- [ ] Publier une histoire technique fondée sur le produit : de documents locaux à un outil de revue, avec source, démo, limites et lien d’essai.
- [ ] Partager dans les communautés pertinentes en respectant leurs règles actuelles et en déclarant être le créateur. Échelonner les publications pour pouvoir répondre et corriger.

### Tableau de bord d’adoption

Point de départ vérifié : 27 stars et 1 fork. Les autres mesures sont à établir ; aucune croissance promise.

| Mesure | Premier objectif de validation | Décision associée |
| --- | --- | --- |
| Compréhension du README | 4 personnes sur 5 expliquent le résultat | Sinon retravailler le message et la démo |
| Activation accompagnée uniquement par les docs | 4 personnes sur 5 lancent leur projet | Sinon corriger P0 avant promotion supplémentaire |
| Utilité | 3 builders poursuivent leur outil une semaine après | Sinon revoir le cas d’usage avec eux |
| Preuve externe | 2 applications partageables | Sinon ne pas revendiquer un écosystème |
| Contributions | 1 première PR externe utile intégrée | Sinon examiner qualité des issues et accueil |
| Attention | Évolution hebdomadaire stars, trafic et téléchargements | Indicateur secondaire, avec limites des compteurs |

Ces seuils sont des objectifs internes pour un petit pilote, pas des statistiques représentatives. Ne pas calculer un taux visite→installation en reliant des compteurs qui ne mesurent pas les mêmes utilisateurs.

## 9. P5 — Ce qui pourrait rendre le produit exceptionnel

N’investir ici qu’après observation de blocages concrets. Chaque expérimentation doit avoir un résultat mesurable.

| Investissement | Valeur attendue | Condition pour le lancer |
| --- | --- | --- |
| Réduire le coût de la première compilation ; étudier un runner précompilé | Premier résultat plus rapide pour les frontend devs | Mesures de #7 montrant que la toolchain est un obstacle majeur ; préserver contrôle des assets et permissions |
| Un excellent template « document desk » | Partir d’un workflow utile au lieu d’une page vide | Builders répétant les mêmes écrans ; réutiliser le registre existant |
| Jobs annulables et progression | Rester utilisable sur de gros dossiers | Blocages natifs mesurés ; partir du design [#6](https://github.com/OthmaneBlial/rustframe/issues/6) |
| Explication visuelle des permissions | Comprendre immédiatement pourquoi une action est permise ou refusée | Confusion observée ; s’appuyer sur les commandes existantes |
| Comparaison reproductible sur une même petite application | Prouver le code de configuration économisé | Même workflow, versions épinglées, coûts de packaging et limites explicités |

Différer mobile, marketplace de plugins, synchronisation cloud, moteur collaboratif et multiplication des applications décoratives. N’ajouter tray, notifications ou updater que si plusieurs vrais projets en dépendent et si la maintenance est soutenable.

## 10. Livraison locale et gates restants

La livraison locale est dans `target/delivery/` : app/DMG macOS ARM, CLI optimisé, API tarball, runtime crate, checksums, inventaires SPDX, notes de release, site avec lecteur fonctionnel et vidéo réelle. Le protocole builders et un article technique sont préparés dans `docs/launch/`. Aucune invitation ni publication de ces contenus n’a eu lieu.

Les cases publiques restent ouvertes pour des raisons concrètes : droits de publication npm/crates.io, signatures Apple/Windows, validation native des autres OS, retours de personnes externes et autorisation d’hébergement des médias. Les investissements P5 restent conditionnés à ces retours.

Le push direct sur `main` a été tenté et refusé par GitHub. Après autorisation de poursuivre par le chemin nécessaire, les commits ont été poussés sur `roadmap-native-delivery` et la [PR #23](https://github.com/OthmaneBlial/rustframe/pull/23) a été ouverte. La fusion automatique est activée ; les six checks obligatoires et la revue indépendante exigée par les règles doivent réussir. Les protections restent inchangées.

### Après résolution des accès externes

1. Relever les versions publiques exactes des trois packages et le dernier état des workflows.
2. Résoudre P0 et produire les reçus de quickstart complets.
3. Valider le scénario Research Desk ; corriger uniquement les défauts bloquants observés.
4. Enregistrer et monter la vraie vidéo avec FFmpeg selon P2.
5. Préparer puis vérifier les releases et la campagne de découverte.

**Priorité directrice : chaque amélioration doit aider une personne extérieure à comprendre, essayer, réussir ou partager RustFrame.**
