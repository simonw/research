Automatically assigning meaningful tags to historic, untagged blog posts, this project leverages the [Simon Willison blog database](https://datasette.simonwillison.net/simonwillisonblog.db) and scikit-learn to train and compare multi-label text classification models. Four approaches—TF-IDF + Logistic Regression, Multinomial Naive Bayes, Random Forest, and LinearSVC—were tested on posts’ title and body text using the 158 most frequently used tags. LinearSVC, with probability calibration, yielded the best overall performance, striking a balance between precision (85%) and recall (56%) with an F1 score of 68%, proving especially effective for assigning multiple tags to each entry. This [open-source toolkit](https://scikit-learn.org/) not only automates metadata enrichment but facilitates rapid quality assessment and scalable tag prediction for content libraries.

**Key findings:**
- LinearSVC outperformed other models, delivering the highest F1 score (0.6791) and recall.
- Logistic Regression and Random Forest prioritized precision but were more conservative—missing more actual tags.
- Naive Bayes offered a fast, simple solution with a solid balance of metrics.
- TF-IDF features and OneVsRest multi-label strategies proved robust for text classification in high-dimensional spaces.
