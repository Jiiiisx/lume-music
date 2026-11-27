import ExpoModulesCore
import Foundation
import MediaPlayer

public class LumeCoreModule: Module {
  private var coreInstance: LumeMobileCore?
  private let coreLock = NSLock()
  private let workerQueue = DispatchQueue(label: "com.lumemusic.core.worker", qos: .userInitiated, attributes: .concurrent)
  private var remoteCommandsConfigured = false

  private func getCore() throws -> LumeMobileCore {
    coreLock.lock()
    defer { coreLock.unlock() }

    if let existing = coreInstance {
      return existing
    }

    let fm = FileManager.default
    let appSupport = try fm.url(
      for: .applicationSupportDirectory,
      in: .userDomainMask,
      appropriateFor: nil,
      create: true
    ).appendingPathComponent("LumeMusic", isDirectory: true)
    let caches = try fm.url(
      for: .cachesDirectory,
      in: .userDomainMask,
      appropriateFor: nil,
      create: true
    ).appendingPathComponent("LumeMusic", isDirectory: true)

    try fm.createDirectory(at: appSupport, withIntermediateDirectories: true)
    try fm.createDirectory(at: caches, withIntermediateDirectories: true)

    let created = try LumeMobileCore(dataDir: appSupport.path, cacheDir: caches.path)
    coreInstance = created
    return created
  }

  private func runAsync(_ promise: Promise, _ work: @escaping (LumeMobileCore) throws -> String) {
    workerQueue.async {
      do {
        let core = try self.getCore()
        let result = try work(core)
        promise.resolve(result)
      } catch {
        promise.reject("LUME_CORE_ERROR", error.localizedDescription)
      }
    }
  }

  private func runAsyncDouble(_ promise: Promise, _ work: @escaping (LumeMobileCore) throws -> Double) {
    workerQueue.async {
      do {
        let core = try self.getCore()
        let result = try work(core)
        promise.resolve(result)
      } catch {
        promise.reject("LUME_CORE_ERROR", error.localizedDescription)
      }
    }
  }

  private func runAsyncVoid(_ promise: Promise, _ work: @escaping (LumeMobileCore) throws -> Void) {
    workerQueue.async {
      do {
        let core = try self.getCore()
        try work(core)
        promise.resolve(nil)
      } catch {
        promise.reject("LUME_CORE_ERROR", error.localizedDescription)
      }
    }
  }

  public func definition() -> ModuleDefinition {
    Name("LumeCore")

    Events("onRemoteNextTrack", "onRemotePreviousTrack")

    Function("enableRemoteTrackCommands") {
      DispatchQueue.main.async {
        if self.remoteCommandsConfigured {
          return
        }
        self.remoteCommandsConfigured = true
        let center = MPRemoteCommandCenter.shared()

        center.nextTrackCommand.isEnabled = true
        center.nextTrackCommand.addTarget { [weak self] _ in
          self?.sendEvent("onRemoteNextTrack", [:])
          return .success
        }

        center.previousTrackCommand.isEnabled = true
        center.previousTrackCommand.addTarget { [weak self] _ in
          self?.sendEvent("onRemotePreviousTrack", [:])
          return .success
        }
      }
    }

    AsyncFunction("healthCheck") { (promise: Promise) in
      self.runAsync(promise) { core in
        try core.healthCheckJson()
      }
    }

    AsyncFunction("resolvePlayableTrack") { (trackJson: String, forceRematch: Bool, promise: Promise) in
      self.runAsync(promise) { core in
        try core.resolvePlayableTrackJson(trackJson: trackJson, forceRematch: forceRematch)
      }
    }

    AsyncFunction("downloadTrackOffline") { (trackJson: String, promise: Promise) in
      self.runAsync(promise) { core in
        try core.downloadTrackOfflineJson(trackJson: trackJson)
      }
    }

    AsyncFunction("listOfflineTracks") { (promise: Promise) in
      self.runAsync(promise) { core in
        try core.listOfflineTracksJson()
      }
    }

    AsyncFunction("deleteOfflineTrack") { (trackUid: String, promise: Promise) in
      self.runAsyncVoid(promise) { core in
        try core.deleteOfflineTrack(trackUid: trackUid)
      }
    }

    AsyncFunction("offlineStorageBytes") { (promise: Promise) in
      self.runAsyncDouble(promise) { core in
        Double(try core.offlineStorageBytes())
      }
    }

    AsyncFunction("searchYtm") { (query: String, limit: Int, promise: Promise) in
      let cap = UInt32(max(1, min(limit, 50)))
      self.runAsync(promise) { core in
        try core.searchYtmJson(query: query, limit: cap)
      }
    }

    AsyncFunction("getCharts") { (indonesia: Bool, limit: Int, forceRefresh: Bool, promise: Promise) in
      let cap = UInt32(max(1, min(limit, 50)))
      self.runAsync(promise) { core in
        try core.getChartsJson(indonesia: indonesia, limit: cap, forceRefresh: forceRefresh)
      }
    }

    AsyncFunction("getArtistProfile") { (artistId: String, promise: Promise) in
      self.runAsync(promise) { core in
        try core.getArtistProfileJson(artistId: artistId)
      }
    }

    AsyncFunction("getAlbumDetail") { (albumId: String, promise: Promise) in
      self.runAsync(promise) { core in
        try core.getAlbumDetailJson(albumId: albumId)
      }
    }

    AsyncFunction("getYtmPlaylistDetail") { (playlistId: String, promise: Promise) in
      self.runAsync(promise) { core in
        try core.getYtmPlaylistDetailJson(playlistId: playlistId)
      }
    }

    AsyncFunction("listPlaylists") { (promise: Promise) in
      self.runAsync(promise) { core in
        try core.listPlaylistsJson()
      }
    }

    AsyncFunction("createPlaylist") { (name: String, promise: Promise) in
      self.runAsync(promise) { core in
        try core.createPlaylistJson(name: name)
      }
    }

    AsyncFunction("deletePlaylist") { (playlistId: String, promise: Promise) in
      self.runAsyncVoid(promise) { core in
        try core.deletePlaylist(playlistId: playlistId)
      }
    }

    AsyncFunction("getPlaylistTracks") { (playlistId: String, promise: Promise) in
      self.runAsync(promise) { core in
        try core.getPlaylistTracksJson(playlistId: playlistId)
      }
    }

    AsyncFunction("addTrackToPlaylist") { (playlistId: String, trackJson: String, promise: Promise) in
      self.runAsyncVoid(promise) { core in
        try core.addTrackToPlaylist(playlistId: playlistId, trackJson: trackJson)
      }
    }

    AsyncFunction("removeTrackFromPlaylist") { (playlistId: String, position: Int, promise: Promise) in
      let pos = UInt32(max(0, position))
      self.runAsyncVoid(promise) { core in
        try core.removeTrackFromPlaylist(playlistId: playlistId, position: pos)
      }
    }

    AsyncFunction("setTrackLiked") { (trackJson: String, liked: Bool, promise: Promise) in
      self.runAsyncVoid(promise) { core in
        try core.setTrackLiked(trackJson: trackJson, liked: liked)
      }
    }

    AsyncFunction("listLikedTracks") { (promise: Promise) in
      self.runAsync(promise) { core in
        try core.listLikedTracksJson()
      }
    }

    AsyncFunction("recordPlayHistory") { (trackJson: String, promise: Promise) in
      self.runAsyncVoid(promise) { core in
        try core.recordPlayHistory(trackJson: trackJson)
      }
    }

    AsyncFunction("listRecentTracks") { (limit: Int, promise: Promise) in
      let cap = UInt32(max(1, min(limit, 100)))
      self.runAsync(promise) { core in
        try core.listRecentTracksJson(limit: cap)
      }
    }

    AsyncFunction("fetchLyrics") { (trackJson: String, forceRefresh: Bool, promise: Promise) in
      self.runAsync(promise) { core in
        try core.fetchLyricsJson(trackJson: trackJson, forceRefresh: forceRefresh)
      }
    }

    AsyncFunction("beginSpotifyMobileAuth") { (clientId: String, promise: Promise) in
      self.runAsync(promise) { core in
        try core.beginSpotifyMobileAuthJson(clientId: clientId)
      }
    }

    AsyncFunction("exchangeSpotifyMobileCode") { (clientId: String, code: String, codeVerifier: String, promise: Promise) in
      self.runAsync(promise) { core in
        try core.exchangeSpotifyMobileCodeJson(
          clientId: clientId,
          code: code,
          codeVerifier: codeVerifier
        )
      }
    }

    AsyncFunction("refreshSpotifyToken") { (clientId: String, tokenJson: String, promise: Promise) in
      self.runAsync(promise) { core in
        try core.refreshSpotifyTokenJson(clientId: clientId, tokenJson: tokenJson)
      }
    }

    AsyncFunction("syncSpotifyLibrary") { (accessToken: String, promise: Promise) in
      self.runAsync(promise) { core in
        try core.syncSpotifyLibraryJson(accessToken: accessToken)
      }
    }

    AsyncFunction("importSpotifyPlaylistUrl") { (url: String, promise: Promise) in
      self.runAsync(promise) { core in
        try core.importSpotifyPlaylistUrlJson(url: url)
      }
    }

    AsyncFunction("rematchSpotifyTrack") { (spotifyTrackId: String, ytVideoId: String, promise: Promise) in
      self.runAsyncVoid(promise) { core in
        try core.rematchSpotifyTrack(spotifyTrackId: spotifyTrackId, ytVideoId: ytVideoId)
      }
    }
  }
}
