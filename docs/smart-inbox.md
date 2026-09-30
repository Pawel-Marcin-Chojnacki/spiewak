# Smart Inbox — future milestone

Not implemented in the first playback slice. Keep automation disabled until manual metadata handling and reversible operation history are reliable.

Future pipeline: watch configured inboxes, wait for stable files, validate, read tags, remove only known filename noise, match/fingerprint against trustworthy providers, compute configurable confidence, propose/apply changes, record reversible history. Existing albums must never be silently moved or reorganized. No metadata may be invented for unresolved files.

Provider credentials must be configured securely outside the repository. Low confidence belongs in Needs Review. Each automatic rename/tag/artwork mutation needs a recoverable original state and a tested Undo path before enabling automatic apply.
