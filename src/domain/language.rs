/// A submission language entry as exposed by an online judge. The `id` is the
/// opaque token the judge expects in the submission form (e.g. AtCoder's
/// numeric `LanguageId` like `5001`); `name` is the human-readable label
/// scraped from the language `<select>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Language {
    pub id: String,
    pub name: String,
}
