#!/usr/bin/env python3
"""
Model 3: TF-IDF + Random Forest

This model uses TF-IDF vectorization and applies Random Forest classifier
for multi-label classification. Random Forest is an ensemble method that
builds multiple decision trees and merges them for better predictions.
"""

import json
import time
from sklearn.feature_extraction.text import TfidfVectorizer
from sklearn.ensemble import RandomForestClassifier
from sklearn.multiclass import OneVsRestClassifier
from sklearn.preprocessing import MultiLabelBinarizer
from sklearn.model_selection import train_test_split
from sklearn.metrics import hamming_loss, f1_score, precision_score, recall_score
import numpy as np

def train_and_predict():
    print("=" * 70)
    print("Model 3: TF-IDF + Random Forest")
    print("=" * 70)

    # Load prepared data
    print("\nLoading prepared data...")
    with open('prepared_data.json', 'r') as f:
        data = json.load(f)

    training_data = data['training']
    prediction_data = data['prediction']
    print(f"Training samples: {len(training_data)}")
    print(f"Prediction samples: {len(prediction_data)}")

    # Extract texts and labels
    X_train_full = [item['text'] for item in training_data]
    y_train_full = [item['filtered_tags'] for item in training_data]

    # Split for validation
    X_train, X_val, y_train, y_val = train_test_split(
        X_train_full, y_train_full, test_size=0.2, random_state=42
    )

    # Binarize labels
    mlb = MultiLabelBinarizer()
    y_train_bin = mlb.fit_transform(y_train)
    y_val_bin = mlb.transform(y_val)

    print(f"\nNumber of unique tags: {len(mlb.classes_)}")
    print(f"Training set: {len(X_train)} samples")
    print(f"Validation set: {len(X_val)} samples")

    # TF-IDF Vectorization
    print("\nVectorizing text with TF-IDF...")
    start_time = time.time()
    vectorizer = TfidfVectorizer(
        max_features=3000,  # Reduced for Random Forest efficiency
        min_df=2,
        max_df=0.8,
        ngram_range=(1, 2),
        stop_words='english'
    )
    X_train_tfidf = vectorizer.fit_transform(X_train)
    X_val_tfidf = vectorizer.transform(X_val)
    print(f"Vectorization completed in {time.time() - start_time:.2f}s")
    print(f"Feature matrix shape: {X_train_tfidf.shape}")

    # Train model
    print("\nTraining Random Forest model...")
    start_time = time.time()
    model = OneVsRestClassifier(
        RandomForestClassifier(
            n_estimators=100,
            max_depth=20,
            min_samples_split=5,
            random_state=42,
            n_jobs=-1
        ),
        n_jobs=1  # Random Forest already uses n_jobs
    )
    model.fit(X_train_tfidf, y_train_bin)
    training_time = time.time() - start_time
    print(f"Training completed in {training_time:.2f}s")

    # Validate
    print("\nValidating model...")
    y_val_pred = model.predict(X_val_tfidf)
    y_val_pred_proba = model.predict_proba(X_val_tfidf)

    # Calculate metrics
    hamming = hamming_loss(y_val_bin, y_val_pred)
    f1_micro = f1_score(y_val_bin, y_val_pred, average='micro', zero_division=0)
    f1_macro = f1_score(y_val_bin, y_val_pred, average='macro', zero_division=0)
    precision = precision_score(y_val_bin, y_val_pred, average='micro', zero_division=0)
    recall = recall_score(y_val_bin, y_val_pred, average='micro', zero_division=0)

    print(f"\nValidation Metrics:")
    print(f"  Hamming Loss: {hamming:.4f}")
    print(f"  F1 Score (micro): {f1_micro:.4f}")
    print(f"  F1 Score (macro): {f1_macro:.4f}")
    print(f"  Precision (micro): {precision:.4f}")
    print(f"  Recall (micro): {recall:.4f}")

    # Predict on entries without tags
    print("\nPredicting tags for entries without tags...")
    X_pred = [item['text'] for item in prediction_data]
    X_pred_tfidf = vectorizer.transform(X_pred)

    start_time = time.time()
    y_pred = model.predict(X_pred_tfidf)
    y_pred_proba = model.predict_proba(X_pred_tfidf)
    prediction_time = time.time() - start_time
    print(f"Prediction completed in {prediction_time:.2f}s")

    # Prepare results
    results = []
    for i, item in enumerate(prediction_data):
        predicted_tags = [tag for tag, val in zip(mlb.classes_, y_pred[i]) if val == 1]

        # Get top 10 tags by probability
        tag_probas = [(tag, prob) for tag, prob in zip(mlb.classes_, y_pred_proba[i])]
        tag_probas.sort(key=lambda x: x[1], reverse=True)
        top_tags_with_proba = tag_probas[:10]

        results.append({
            'id': item['id'],
            'title': item['title'],
            'created': item['created'],
            'predicted_tags': predicted_tags,
            'top_tags_with_probability': [
                {'tag': tag, 'probability': float(prob)}
                for tag, prob in top_tags_with_proba
            ]
        })

    # Save results
    output = {
        'model_name': 'TF-IDF + Random Forest',
        'model_description': 'Uses TF-IDF vectorization with Random Forest classifier for multi-label classification. Random Forest builds multiple decision trees and averages their predictions for robustness.',
        'hyperparameters': {
            'tfidf_max_features': 3000,
            'tfidf_ngram_range': [1, 2],
            'tfidf_min_df': 2,
            'tfidf_max_df': 0.8,
            'rf_n_estimators': 100,
            'rf_max_depth': 20,
            'rf_min_samples_split': 5
        },
        'training_info': {
            'training_samples': len(X_train),
            'validation_samples': len(X_val),
            'num_tags': len(mlb.classes_),
            'num_features': X_train_tfidf.shape[1],
            'training_time_seconds': training_time,
            'prediction_time_seconds': prediction_time
        },
        'validation_metrics': {
            'hamming_loss': float(hamming),
            'f1_score_micro': float(f1_micro),
            'f1_score_macro': float(f1_macro),
            'precision_micro': float(precision),
            'recall_micro': float(recall)
        },
        'predictions': results,
        'tag_classes': mlb.classes_.tolist()
    }

    with open('results_random_forest.json', 'w') as f:
        json.dump(output, f, indent=2)

    print(f"\nResults saved to results_random_forest.json")
    print(f"Predicted tags for {len(results)} entries")

    # Show some examples
    print("\nSample predictions:")
    for i in range(min(3, len(results))):
        print(f"\n  Entry {i+1}: {results[i]['title'][:60]}...")
        print(f"  Predicted: {', '.join(results[i]['predicted_tags'][:5])}")
        print(f"  Top probabilities:")
        for tag_prob in results[i]['top_tags_with_probability'][:3]:
            print(f"    - {tag_prob['tag']}: {tag_prob['probability']:.3f}")

    return output

if __name__ == '__main__':
    train_and_predict()
