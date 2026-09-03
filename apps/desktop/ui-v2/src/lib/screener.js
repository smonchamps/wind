// The display vocabulary for the Screener's verdicts — ONE copy
// (RETOURS-14 R5, review: Screener.svelte and Settings.svelte each
// carried their own; a No rule added on one side would have left
// the other surface fall back in silence to the generic label). The
// keys mirror the CLOSED vocabulary of the core (`routage_expediteurs`),
// like `vocabularies.js` for the horizons.
export const SCREENED_OUT_LABEL = {
  spam: 'screener.screenedOutSpam',
  archive: 'screener.screenedOutArchive',
  trash: 'screener.screenedOutTrash',
};

export const DESTINATION_LABEL = {
  inbox: 'screener.toInbox',
  feed: 'screener.toFeed',
  paper_trail: 'screener.toPaperTrail',
};
