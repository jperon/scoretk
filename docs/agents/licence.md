# Licence : obligations d'un consommateur

Ce document décrit les obligations de quiconque **utilise ou distribue**
`mscz2xml` — la bibliothèque Rust, le binaire `mscz2xml` ou le paquet
WebAssembly (`nix build .#wasm`) — et non celles du projet lui-même (voir
`NOTICE`). Le projet est sous **GPL-3.0-only** ; il n'existe **aucune licence
plus permissive** pour la bibliothèque, y compris pour le wasm.

> Ce texte est une aide technique, pas un avis juridique. Pour un produit
> commercial, faire relire le montage par un juriste.

## Utilisation simple : aucune obligation

La GPL v3 ne restreint ni l'usage privé, ni l'usage interne en entreprise, ni
l'exécution : utiliser `mscz2xml` pour convertir ses propres partitions ne
déclenche rien. De même, un service en ligne qui **exécute** le binaire ou le
wasm à la demande n'est pas, en soi, une distribution : la GPL v3, contrairement
à l'AGPL, ne comporte pas de clause « usage par réseau ». (La situation change
dès que le code lui-même est livré : assets statiques, paquet npm, application
mobile ou de bureau, CDN public.)

## Le cas d'une application qui embarque le wasm ou la bibliothèque

Charger le module wasm dans une application JavaScript (page, worker, PWA,
extension, application empaquetée) et l'appeler depuis du code non-GPL crée
**vraisemblablement une œuvre combinée** au sens de la GPL : le module et son
code appelant s'exécutent dans le même programme, partagent ses structures de
données et forment un tout fonctionnel. C'est l'analyse habituelle de la FSF
pour l'édition de liens dynamiques, dont le chargement d'un module wasm est
l'équivalent technologique. La « simple agrégation » (§5) ne couvre que des
programmes indépendants, communiquant au mieux par des fichiers ou des
arguments de ligne de commande — pas un module chargé et piloté en mémoire.

Conséquences pour une telle application **distribuée** :

1. **Licence compatible.** L'ensemble (application + wasm) doit être distribué
   sous GPL-3.0-only ou une licence compatible avec elle — en pratique
   GPL-3.0-only, ou GPL-3.0-or-later si l'on veut laisser le choix aux
   destinataires. Une licence permissive (MIT, Apache-2.0) ne suffit pas :
   elle autorise l'usage du wasm, mais l'ensemble distribué reste soumis à la
   GPL.
2. **Source correspondante.** Fournir la « Corresponding Source » de
   l'ensemble : le code de l'application, la version exacte du wasm utilisée,
   et tout ce qui est nécessaire pour le reconstruire à partir de cette
   source (le crate `mscz2xml`, le `Cargo.lock`, la recette de construction —
   ici `flake.nix`, les scripts et la version de wasm-bindgen). La source doit
   être livrée avec le binaire ou proposée par écrit (§6), et rester
   accessible aussi longtemps que le binaire est distribué.
3. **Avis conservés.** Ne pas retirer les en-têtes SPDX, les mentions de
   copyright, `LICENSE` ni `NOTICE` du paquet. Les fichiers modifiés doivent
   porter un avis de modification bien visible (§5a) : si l'on modifie le
   wasm ou son wrapper, l'indiquer avec la date.
4. **Avis légaux dans l'interface.** Pour un programme interactif, afficher
   les « Appropriate Legal Notices » (§0) : au minimum le nom du programme, la
   licence (GPL-3.0-only), l'absence de garantie et un lien vers la source
   correspondante — typiquement dans un écran « À propos » ou « Licence ».
   C'est la contrepartie de l'absence de publicité imposée par la GPL.
5. **Pas de restriction supplémentaire.** Aucune condition ajoutée (EULA,
   DRM empêchant la copie du logiciel, limitation du nombre d'installations,
   clause de non-rétro-ingénierie plus stricte que la loi) ne doit s'ajouter
   aux droits conférés par la GPL (§7, §10). Une clause de résiliation
   unilatérale ou une « licence d'utilisation » séparée est incompatible.
6. **Brevets.** La GPL v3 concède une licence de brevet implicite (§11) ; ne
   pas ajouter de clause qui la contredirait. Inversement, distribuer
   l'ensemble n'oblige pas à céder ses propres brevets au-delà de ce que
   prévoit la GPL.
7. **Marque.** La GPL ne concède aucun droit sur la marque « MuseScore »
   (MuseScore Limited). Une application dérivée peut décrire son format
   d'entrée, mais ne doit pas suggérer une affiliation ni une approbation.

## Cas particuliers

- **Modifier le wasm ou la bibliothèque** : mêmes obligations, plus l'avis de
  modification (§5a) et l'interdiction de renommer ou masquer l'origine. Le
  code modifié reste GPL-3.0-only.
- **Distribuer uniquement le binaire** (`.wasm`, paquet npm, APK) : c'est une
  distribution ; il faut joindre la source correspondante ou une offre écrite
  valable, avec l'URL et la version exacte.
- **Intégrer dans un produit matériel** (firmware d'un appareil) : la §6
  impose aussi les « Installation Information » nécessaires pour installer
  une version modifiée sur l'appareil, dans la mesure où l'appareil est un
  « User Product ».
- **Mélanger avec du code GPL-incompatible** (bibliothèque propriétaire
  liée, code sous GPLv2-only) : la distribution de l'ensemble est impossible
  sans changer de licence ou isoler réellement les programmes (processus
  séparés, échange par fichiers).
- **Utiliser le wasm côté serveur sans le distribuer** : pas d'obligation de
  publication tant que le code n'est pas livré au client. Attention : servir
  les fichiers `.js`/`.wasm` au navigateur est une distribution.

## Résumé pratique

| Action | Obligation |
| --- | --- |
| Convertir ses partitions avec la CLI | aucune |
| Appeler le wasm depuis une page servie à des tiers | distribuer l'application sous GPL-compatible + source correspondante + avis |
| Publier le paquet wasm tel quel sur un CDN ou npm | joindre `LICENSE`, `NOTICE` et la source correspondante (déjà dans le paquet pour les avis ; le dépôt pour la source) |
| Modifier le code | avis de modification daté, source publiée, licence conservée |
| Intégrer dans un firmware/appareil | + Installation Information (§6) |

Le paquet wasm livré par `nix build .#wasm` contient déjà `LICENSE`, `NOTICE`,
le bandeau `/*! @license … */` en tête des fichiers JS et la section
« Licence et sources » de son README ; l'application hôte doit, elle, afficher
la licence et le lien vers sa propre source correspondante.
