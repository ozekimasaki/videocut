# シンプルなタイムラインの FFmpeg フィルタグラフ

VideoCut の書き出しは、1 本の映像トラックと音声トラックのミックスを ffmpeg の `filter_complex` に落とす。対象はハードカット、隙間、クロスディゾルブである。コマンドは `cmd.exe` を通さず、フィルタ本文は `-filter_complex_script` のファイルで渡す。Windows では引数長の上限を避けるためである。

## サイドカー

Windows では `ffmpeg.exe` と `ffprobe.exe` を探す。

1. `VIDEOCUT_FFMPEG` と `VIDEOCUT_FFPROBE`
2. `VIDEOCUT_FFMPEG_DIR`
3. アプリの隣、`ffmpeg/`、`ffmpeg/bin/`、`resources/ffmpeg/`
4. `PATH`

レイアウト例:

```text
VideoCut.exe
ffmpeg\ffmpeg.exe
ffmpeg\ffprobe.exe
```

## カット

各クリップは入力番号で参照する。ソース時間は `trim` / `atrim` で切り、`setpts` / `asetpts` でタイムスタンプをゼロに戻す。画面はシーケンスサイズへ `scale` と `pad` し、`fps` と `format` を揃えてから `concat` する。入力シークは使わない。カット位置をフィルタ側で決める。

1920x1080、30 fps、48 kHz ステレオで、1 秒のクリップを 2 本つないだグラフは次のとおり。

```text
[0:v]trim=start=0:end=1,setpts=PTS-STARTPTS,scale=1920:1080:force_original_aspect_ratio=decrease,pad=1920:1080:(ow-iw)/2:(oh-ih)/2:black,fps=30/1,setsar=1,format=yuv420p[v0];[1:v]trim=start=0:end=1,setpts=PTS-STARTPTS,scale=1920:1080:force_original_aspect_ratio=decrease,pad=1920:1080:(ow-iw)/2:(oh-ih)/2:black,fps=30/1,setsar=1,format=yuv420p[v1];[v0][v1]concat=n=2:v=1:a=0[vout];[0:a]atrim=start=0:end=1,asetpts=PTS-STARTPTS,aformat=sample_fmts=fltp:sample_rates=48000:channel_layouts=stereo[a0];[1:a]atrim=start=0:end=1,asetpts=PTS-STARTPTS,aformat=sample_fmts=fltp:sample_rates=48000:channel_layouts=stereo[a1];[a0][a1]concat=n=2:v=0:a=1[aout]
```

## 隙間とディゾルブ

タイムラインの空きは映像が `color=c=black`、音声が `anullsrc` である。重なりに `builtin.cross-dissolve` があるときは、重なっている区間だけを切り出して映像を `xfade=transition=fade`、音声を `acrossfade` でつなぐ。`offset=0` なのは、両方の入力をすでにディゾルブ尺へ切ってあるためである。それ以外のトランジションはカットとして書き、警告を残す。

重なりがありトランジションがない区間は、後から乗ったクリップを優先する。先のクリップが後ろまで残っていれば、その尾を戻す。

## それ以外

- 静止画は `-loop 1` でタイムライン上の長さだけ出す。
- 速度は映像が `setpts=(PTS-STARTPTS)/速度`、音声が `atempo`（0.5〜2.0 の範囲に分割）である。
- 逆再生は `reverse` と `areverse` である。
- 映像は一番上の表示トラックだけを出す。下のトラックは合成しない。
- ミュートでも非表示でもない音声トラックは `amix` で混ぜる。
- タイトル、キャプション、ネスト、調整レイヤーは焼かず、警告に残す。
- H.264 は `libx264` と AAC の MP4、ProRes は `prores_ks`（なければ `prores_aw`、`prores`）の MOV、音声だけは `pcm_s16le` の WAV である。ProRes エンコーダが無いビルドでは書き出さない。
