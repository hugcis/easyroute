import SwiftUI

struct RegionManagementView: View {
    @Environment(RegionManager.self) var regionManager
    @Environment(\.dismiss) private var dismiss
    var onRegionChanged: (String) -> Void

    var body: some View {
        NavigationStack {
            List {
                currentRegionSection
                onDeviceSection
                availableSection
            }
            .navigationTitle("Regions")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Done") { dismiss() }
                }
            }
            .refreshable { await regionManager.fetchCatalog() }
            .task { await regionManager.fetchCatalog() }
        }
    }

    // MARK: - Current Region

    private var currentRegionSection: some View {
        Section("Current Region") {
            Label(regionManager.activeRegionName, systemImage: "map.fill")
                .font(.body.weight(.medium))
        }
    }

    // MARK: - On Device

    private var onDeviceSection: some View {
        Section("On Device") {
            // Default / bundled region
            Button {
                regionManager.activeRegionId = nil
                onRegionChanged(regionManager.activeRegionPath)
            } label: {
                HStack {
                    Text("Default")
                        .font(.body)
                        .foregroundStyle(.primary)
                    Spacer()
                    if regionManager.activeRegionId == nil {
                        Image(systemName: "checkmark")
                            .foregroundStyle(.blue)
                            .fontWeight(.semibold)
                    }
                }
            }

            ForEach(regionManager.downloadedRegions) { region in
                Button {
                    regionManager.setActiveRegion(id: region.id)
                    onRegionChanged(regionManager.activeRegionPath)
                } label: {
                    HStack {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(region.name)
                                .font(.body)
                                .foregroundStyle(.primary)
                            Text("\(region.formattedPoiCount) POIs  ·  \(region.formattedSize)")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                        Spacer()
                        if regionManager.activeRegionId == region.id {
                            Image(systemName: "checkmark")
                                .foregroundStyle(.blue)
                                .fontWeight(.semibold)
                        }
                    }
                }
                .swipeActions(edge: .trailing) {
                    if regionManager.activeRegionId != region.id {
                        Button(role: .destructive) {
                            regionManager.deleteRegion(id: region.id)
                        } label: {
                            Label("Delete", systemImage: "trash")
                        }
                    }
                }
            }
        }
    }

    @ViewBuilder
    private var availableSection: some View {
        let available = regionManager.catalog.filter { remote in
            !regionManager.downloadedRegions.contains { $0.id == remote.id }
        }

        if regionManager.isFetchingCatalog && regionManager.catalog.isEmpty {
            Section("Available for Download") {
                HStack {
                    ProgressView()
                    Text("Loading catalog...")
                        .foregroundStyle(.secondary)
                }
            }
        } else if let error = regionManager.catalogError, regionManager.catalog.isEmpty {
            Section("Available for Download") {
                Label(error, systemImage: "exclamationmark.triangle")
                    .foregroundStyle(.secondary)
                    .font(.subheadline)
            }
        } else if !available.isEmpty {
            Section("Available for Download") {
                ForEach(available) { region in
                    HStack {
                        VStack(alignment: .leading, spacing: 2) {
                            Text(region.name)
                                .font(.body)
                            Text("\(region.formattedPoiCount) POIs  ·  \(region.formattedSize)")
                                .font(.caption)
                                .foregroundStyle(.secondary)
                        }
                        Spacer()
                        downloadButton(for: region)
                    }
                }
            }
        }
    }

    @ViewBuilder
    private func downloadButton(for region: RemoteRegion) -> some View {
        if let progress = regionManager.downloads[region.id] {
            ProgressView(value: progress)
                .progressViewStyle(.circular)
                .frame(width: 24, height: 24)
        } else {
            Button {
                Task { await regionManager.downloadRegion(region) }
            } label: {
                Image(systemName: "arrow.down.circle")
                    .font(.title3)
            }
        }
    }
}
