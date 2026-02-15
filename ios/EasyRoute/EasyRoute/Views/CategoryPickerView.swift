import SwiftUI

struct CategoryPickerView: View {
    @Binding var selectedCategories: Set<String>
    @Environment(\.dismiss) private var dismiss

    var body: some View {
        NavigationStack {
            categoryList
                .navigationTitle("Categories")
                .navigationBarTitleDisplayMode(.inline)
                .toolbar {
                    ToolbarItem(placement: .topBarLeading) {
                        Menu {
                            Button("Select All") { selectAll() }
                            Button("Clear All") { selectedCategories.removeAll() }
                        } label: {
                            Text(summaryText)
                                .font(.subheadline)
                        }
                    }
                    ToolbarItem(placement: .topBarTrailing) {
                        Button("Done") { dismiss() }
                            .bold()
                    }
                }
        }
    }

    private var categoryList: some View {
        List {
            ForEach(POICategories.groups) { group in
                Section(group.name) {
                    ForEach(group.categories, id: \.key) { item in
                        categoryRow(key: item.key, info: item.info)
                    }
                }
            }
        }
    }

    private func categoryRow(key: String, info: CategoryInfo) -> some View {
        Button {
            toggleCategory(key)
        } label: {
            HStack {
                Image(systemName: info.symbol)
                    .foregroundStyle(info.color)
                    .frame(width: 24)
                Text(info.label)
                    .foregroundStyle(.primary)
                Spacer()
                if isSelected(key) {
                    Image(systemName: "checkmark")
                        .foregroundStyle(Color.accentColor)
                }
            }
        }
    }

    private func isSelected(_ key: String) -> Bool {
        selectedCategories.isEmpty || selectedCategories.contains(key)
    }

    private func toggleCategory(_ key: String) {
        if selectedCategories.isEmpty {
            selectedCategories = POICategories.allCategoryKeys
            selectedCategories.remove(key)
        } else if selectedCategories.contains(key) {
            selectedCategories.remove(key)
        } else {
            selectedCategories.insert(key)
            if selectedCategories == POICategories.allCategoryKeys {
                selectedCategories.removeAll()
            }
        }
    }

    private func selectAll() {
        selectedCategories.removeAll()
    }

    private var summaryText: String {
        if selectedCategories.isEmpty {
            return "All selected"
        }
        return "\(selectedCategories.count) of \(POICategories.allCategoryKeys.count)"
    }
}
