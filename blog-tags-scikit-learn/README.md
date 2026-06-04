# Blog Tag Prediction with Scikit-Learn

<!-- AI-GENERATED-NOTE -->
> [!NOTE]
> This is an AI-generated research report. All text and code in this report was created by an LLM (Large Language Model). For more information on how these reports are created, see the [main research repository](https://github.com/simonw/research).
<!-- /AI-GENERATED-NOTE -->

This project implements multiple machine learning models to predict tags for untagged blog entries from Simon Willison's blog database. The models are trained on blog entries that have tags and then predict tags for older entries that don't have tags yet.

## Table of Contents

- [Overview](#overview)
- [Dataset](#dataset)
- [Models](#models)
- [Installation](#installation)
- [Usage](#usage)
- [Results](#results)
- [File Structure](#file-structure)
- [Methodology](#methodology)
- [Performance Comparison](#performance-comparison)

## Overview

The goal of this project is to automatically suggest relevant tags for historical blog posts that were written before the tagging system was implemented. We use multiple text classification approaches from scikit-learn to accomplish this task.

### Problem Type

This is a **multi-label text classification** problem where:
- Each blog entry can have multiple tags
- We use the title and body text as features
- We predict from a set of 158 frequently-used tags (tags appearing at least 10 times in the dataset)

### Key Statistics

- **Total blog entries:** 3,223
- **Entries with tags (training data):** 2,407
- **Entries without tags (prediction targets):** 816
- **Total unique tags:** 1,290
- **Frequent tags used (≥10 occurrences):** 158
- **Final training samples after filtering:** 2,314

## Dataset

The dataset comes from Simon Willison's blog database, available at:
`https://datasette.simonwillison.net/simonwillisonblog.db`

### Database Schema

Key tables:
- `blog_entry`: Contains blog posts (id, title, body, created, etc.)
- `blog_tag`: Contains tag definitions (id, tag, description)
- `blog_entry_tags`: Many-to-many relationship between entries and tags

### Top Tags in Dataset

1. quora (1,003 entries)
2. projects (267 entries)
3. datasette (240 entries)
4. llms (218 entries)
5. ai (218 entries)
6. python (218 entries)
7. generative-ai (215 entries)
8. weeknotes (193 entries)
9. web-development (166 entries)
10. startups (157 entries)

## Models

We implemented four different machine learning approaches, all using TF-IDF (Term Frequency-Inverse Document Frequency) for text vectorization:

### Model 1: TF-IDF + Logistic Regression

**Description:** Uses Logistic Regression with a OneVsRest wrapper for multi-label classification.

**Hyperparameters:**
- TF-IDF max features: 5,000
- N-gram range: (1, 2)
- Min document frequency: 2
- Max document frequency: 0.8
- Logistic Regression C: 1.0
- Max iterations: 1,000

**Strengths:**
- Very high precision (92%)
- Fast training and prediction
- Good for when you want confident predictions

**Weaknesses:**
- Lower recall (30%)
- Conservative in assigning tags

### Model 2: TF-IDF + Multinomial Naive Bayes

**Description:** Uses Multinomial Naive Bayes, which is based on Bayes' theorem and assumes feature independence.

**Hyperparameters:**
- TF-IDF max features: 5,000
- N-gram range: (1, 2)
- Min document frequency: 2
- Max document frequency: 0.8
- Naive Bayes alpha: 0.1

**Strengths:**
- Best balance of precision and recall
- Highest F1 score
- Fast training
- Works well with text classification

**Weaknesses:**
- Assumes feature independence (may not be true for text)

### Model 3: TF-IDF + Random Forest

**Description:** Ensemble method that builds multiple decision trees and averages their predictions.

**Hyperparameters:**
- TF-IDF max features: 3,000 (reduced for efficiency)
- N-gram range: (1, 2)
- Min document frequency: 2
- Max document frequency: 0.8
- Number of estimators: 100
- Max depth: 20
- Min samples split: 5

**Strengths:**
- High precision (90%)
- Robust to overfitting
- Handles non-linear relationships

**Weaknesses:**
- Slower training (33.7s)
- Slower prediction (11.2s)
- Lower recall (37%)

### Model 4: TF-IDF + LinearSVC

**Description:** Support Vector Classification with calibration for probability estimates.

**Hyperparameters:**
- TF-IDF max features: 5,000
- N-gram range: (1, 2)
- Min document frequency: 2
- Max document frequency: 0.8
- SVC C: 1.0
- Max iterations: 1,000
- Calibration CV: 3

**Strengths:**
- **Best overall performance** (highest F1 score: 68%)
- Best recall (56%)
- Good precision (85%)
- Effective for high-dimensional text data

**Weaknesses:**
- Slightly slower than Logistic Regression
- Requires calibration for probability estimates

## Installation

### Prerequisites

- Python 3.7+
- pip

### Install Dependencies

```bash
pip install pandas scikit-learn numpy
```

Or install all dependencies at once:

```bash
pip install -r requirements.txt
```

## Usage

### Quick Start

Run all models at once:

```bash
python3 run_all_models.py
```

This will:
1. Prepare the data
2. Run all four models
3. Generate JSON results for each model
4. Create a comparison report

### Individual Model Execution

You can also run models individually:

```bash
# Prepare data first
python3 prepare_data.py

# Run individual models
python3 model_logistic_regression.py
python3 model_naive_bayes.py
python3 model_random_forest.py
python3 model_linear_svc.py
```

### Explore the Database

To explore the database structure and statistics:

```bash
python3 explore_db.py
```

## Results

All models generate JSON output files containing:
- Model name and description
- Hyperparameters used
- Training information (samples, features, time)
- Validation metrics (F1, precision, recall, hamming loss)
- Predictions for all 816 untagged entries
- Top 10 tags with probability scores for each entry

### Output Files

- `results_logistic_regression.json` - Logistic Regression predictions
- `results_naive_bayes.json` - Naive Bayes predictions
- `results_random_forest.json` - Random Forest predictions
- `results_linear_svc.json` - LinearSVC predictions
- `model_comparison.json` - Comparison of all models
- `prepared_data.json` - Prepared training and prediction data

### Visualization

Open `visualization.html` in a web browser to see:
- Interactive comparison of model performance
- Sample predictions from each model
- Tag distribution analysis
- Detailed metrics breakdown

## File Structure

```
blog-tags-scikit-learn/
├── simonwillisonblog.db           # SQLite database (downloaded)
├── README.md                       # This file
├── explore_db.py                   # Database exploration script
├── prepare_data.py                 # Data preparation script
├── model_logistic_regression.py    # Model 1 implementation
├── model_naive_bayes.py            # Model 2 implementation
├── model_random_forest.py          # Model 3 implementation
├── model_linear_svc.py             # Model 4 implementation
├── run_all_models.py               # Main script to run all models
├── visualization.html              # Interactive results visualization
├── prepared_data.json              # Prepared data (generated)
├── results_logistic_regression.json # Model 1 results (generated)
├── results_naive_bayes.json        # Model 2 results (generated)
├── results_random_forest.json      # Model 3 results (generated)
├── results_linear_svc.json         # Model 4 results (generated)
└── model_comparison.json           # Model comparison (generated)
```

## Methodology

### 1. Data Preparation

1. Extract all blog entries with tags (2,407 entries)
2. Extract all blog entries without tags (816 entries)
3. Filter to tags appearing at least 10 times (158 tags)
4. Combine title and body text for each entry
5. Split tagged entries into train (80%) and validation (20%) sets

### 2. Feature Extraction

We use TF-IDF (Term Frequency-Inverse Document Frequency) vectorization:

- **TF (Term Frequency):** How often a word appears in a document
- **IDF (Inverse Document Frequency):** How rare a word is across all documents
- **TF-IDF:** Product of TF and IDF, giving higher weight to important, distinctive words

Configuration:
- Max features: 3,000-5,000 (most important features)
- N-grams: Unigrams and bigrams (1-2 word phrases)
- Min DF: 2 (word must appear in at least 2 documents)
- Max DF: 0.8 (ignore words in more than 80% of documents)
- Stop words: English stop words removed

### 3. Multi-Label Classification

Since each blog entry can have multiple tags, we use:

- **MultiLabelBinarizer:** Converts tag lists to binary vectors
- **OneVsRest Strategy:** Trains one classifier per tag
- **Probability Calibration:** Provides confidence scores for predictions

### 4. Evaluation Metrics

- **Hamming Loss:** Fraction of wrong labels (lower is better)
- **F1 Score (micro):** Harmonic mean of precision and recall
- **Precision (micro):** Of predicted tags, how many are correct?
- **Recall (micro):** Of actual tags, how many did we find?

## Performance Comparison

### Model Performance Summary

| Model | F1 Score | Precision | Recall | Training Time |
|-------|----------|-----------|---------|---------------|
| **LinearSVC** | **0.6791** | **0.8543** | **0.5636** | 3.84s |
| Naive Bayes | 0.5604 | 0.6134 | 0.5159 | 3.35s |
| Random Forest | 0.5231 | 0.9011 | 0.3685 | 33.71s |
| Logistic Regression | 0.4579 | 0.9194 | 0.3049 | 4.62s |

### Winner: LinearSVC

The **TF-IDF + LinearSVC** model performs best overall with:
- Highest F1 score (0.6791)
- Best recall (0.5636) - finds more relevant tags
- Strong precision (0.8543) - makes accurate predictions
- Fast training time (3.84s)

### When to Use Each Model

**Use LinearSVC when:**
- You want the best overall performance
- You need a good balance of precision and recall
- You're working with high-dimensional text data

**Use Logistic Regression when:**
- You need very high precision (92%)
- You prefer conservative, confident predictions
- Speed is critical

**Use Naive Bayes when:**
- You want good overall performance with fast training
- You're working with limited computational resources
- You need a simple, interpretable model

**Use Random Forest when:**
- You need very high precision (90%)
- You suspect non-linear relationships in the data
- Training time is not a concern

## Example Predictions

### Entry: "Netscape 4 is 5 years old"

- **LinearSVC:** programming, jeffrey-zeldman, startups
- **Naive Bayes:** css, mozilla, mark-pilgrim
- **Random Forest:** google, apis, mark-pilgrim
- **Logistic Regression:** quora, python, javascript

### Entry: "Charity and Amazon"

- **LinearSVC:** information-architecture, projects, startups
- **Naive Bayes:** quora, php, blogging
- **Random Forest:** information-architecture, python, php
- **Logistic Regression:** quora, python, startups

## Technical Details

### Why TF-IDF?

TF-IDF is chosen because:
1. It captures word importance beyond simple frequency
2. It works well with sparse, high-dimensional text data
3. It's computationally efficient
4. It's interpretable (we can see which words matter)

### Why These Models?

1. **Logistic Regression:** Standard baseline for text classification
2. **Naive Bayes:** Classic text classification algorithm, very fast
3. **Random Forest:** Ensemble method, robust to overfitting
4. **LinearSVC:** State-of-the-art for high-dimensional text data

### Multi-Label Strategy

We use **OneVsRest** (also called One-vs-All):
- Trains one binary classifier per tag
- Each classifier learns: "Does this entry have this tag or not?"
- Final prediction combines all binary classifiers
- Allows independent tag probabilities

## Future Improvements

Potential enhancements:
1. **Feature engineering:** Add metadata features (date, length, etc.)
2. **Deep learning:** Try neural networks (LSTM, BERT)
3. **Ensemble methods:** Combine multiple models
4. **Hyperparameter tuning:** Grid search for optimal parameters
5. **Tag embeddings:** Use tag co-occurrence patterns
6. **Threshold optimization:** Adjust probability thresholds per tag
7. **Active learning:** Have humans label uncertain predictions

## References

- [Scikit-learn Documentation](https://scikit-learn.org/)
- [TF-IDF Explanation](https://en.wikipedia.org/wiki/Tf%E2%80%93idf)
- [Multi-label Classification](https://en.wikipedia.org/wiki/Multi-label_classification)
- [Simon Willison's Blog](https://simonwillison.net/)

## License

This is a research project for exploring machine learning techniques on blog data.

## Author

Created as a demonstration of scikit-learn's text classification capabilities.
