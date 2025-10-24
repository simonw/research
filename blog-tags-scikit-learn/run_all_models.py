#!/usr/bin/env python3
"""
Main script to run all blog tag prediction models.

This script runs all four models sequentially and generates a comparison
report showing the performance of each model.
"""

import json
import time
import sys

# Import all model modules
import model_logistic_regression
import model_naive_bayes
import model_random_forest
import model_linear_svc

def run_all_models():
    """Run all models and generate comparison report."""
    print("\n" + "=" * 70)
    print("BLOG TAG PREDICTION - RUNNING ALL MODELS")
    print("=" * 70)
    print("\nThis will run 4 different machine learning models:")
    print("  1. TF-IDF + Logistic Regression")
    print("  2. TF-IDF + Multinomial Naive Bayes")
    print("  3. TF-IDF + Random Forest")
    print("  4. TF-IDF + LinearSVC")
    print("\n" + "=" * 70 + "\n")

    results = []
    total_start_time = time.time()

    # Model 1: Logistic Regression
    try:
        print("\n[1/4] Running Logistic Regression...")
        result = model_logistic_regression.train_and_predict()
        results.append(result)
        print("\n✓ Logistic Regression completed")
    except Exception as e:
        print(f"\n✗ Logistic Regression failed: {e}")
        sys.exit(1)

    # Model 2: Naive Bayes
    try:
        print("\n[2/4] Running Naive Bayes...")
        result = model_naive_bayes.train_and_predict()
        results.append(result)
        print("\n✓ Naive Bayes completed")
    except Exception as e:
        print(f"\n✗ Naive Bayes failed: {e}")
        sys.exit(1)

    # Model 3: Random Forest
    try:
        print("\n[3/4] Running Random Forest...")
        result = model_random_forest.train_and_predict()
        results.append(result)
        print("\n✓ Random Forest completed")
    except Exception as e:
        print(f"\n✗ Random Forest failed: {e}")
        sys.exit(1)

    # Model 4: LinearSVC
    try:
        print("\n[4/4] Running LinearSVC...")
        result = model_linear_svc.train_and_predict()
        results.append(result)
        print("\n✓ LinearSVC completed")
    except Exception as e:
        print(f"\n✗ LinearSVC failed: {e}")
        sys.exit(1)

    total_time = time.time() - total_start_time

    # Generate comparison report
    print("\n" + "=" * 70)
    print("MODEL COMPARISON SUMMARY")
    print("=" * 70)

    comparison = {
        'models': [],
        'summary': {
            'total_execution_time_seconds': total_time,
            'num_models': len(results),
            'prediction_entries': results[0]['training_info']['training_samples'] + results[0]['training_info']['validation_samples']
        }
    }

    print(f"\n{'Model':<40} {'F1 (micro)':<12} {'Precision':<12} {'Recall':<12} {'Train Time':<12}")
    print("-" * 88)

    for result in results:
        model_info = {
            'model_name': result['model_name'],
            'validation_metrics': result['validation_metrics'],
            'training_time': result['training_info']['training_time_seconds'],
            'prediction_time': result['training_info']['prediction_time_seconds']
        }
        comparison['models'].append(model_info)

        metrics = result['validation_metrics']
        train_time = result['training_info']['training_time_seconds']

        print(f"{result['model_name'][:39]:<40} "
              f"{metrics['f1_score_micro']:<12.4f} "
              f"{metrics['precision_micro']:<12.4f} "
              f"{metrics['recall_micro']:<12.4f} "
              f"{train_time:<12.2f}s")

    print("-" * 88)

    # Find best model
    best_f1_model = max(results, key=lambda x: x['validation_metrics']['f1_score_micro'])
    print(f"\nBest F1 Score (micro): {best_f1_model['model_name']}")
    print(f"  F1: {best_f1_model['validation_metrics']['f1_score_micro']:.4f}")

    best_precision_model = max(results, key=lambda x: x['validation_metrics']['precision_micro'])
    print(f"\nBest Precision (micro): {best_precision_model['model_name']}")
    print(f"  Precision: {best_precision_model['validation_metrics']['precision_micro']:.4f}")

    best_recall_model = max(results, key=lambda x: x['validation_metrics']['recall_micro'])
    print(f"\nBest Recall (micro): {best_recall_model['model_name']}")
    print(f"  Recall: {best_recall_model['validation_metrics']['recall_micro']:.4f}")

    comparison['best_models'] = {
        'best_f1': best_f1_model['model_name'],
        'best_precision': best_precision_model['model_name'],
        'best_recall': best_recall_model['model_name']
    }

    # Save comparison
    with open('model_comparison.json', 'w') as f:
        json.dump(comparison, f, indent=2)

    print(f"\n{'='*70}")
    print(f"Total execution time: {total_time:.2f}s")
    print(f"\nResults saved:")
    print(f"  - results_logistic_regression.json")
    print(f"  - results_naive_bayes.json")
    print(f"  - results_random_forest.json")
    print(f"  - results_linear_svc.json")
    print(f"  - model_comparison.json")
    print(f"{'='*70}\n")

if __name__ == '__main__':
    run_all_models()
